#!/usr/bin/env python3

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import os
import re
import shutil
import socket
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from shlex import quote, split
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
CLIENT_MAIN = REPO_ROOT / "vpn-client" / "src" / "main.rs"
CLIENT_CLI = REPO_ROOT / "vpn-client" / "src" / "cli.rs"
DEFAULT_CLIENT_HTTP_PORT = 4536
REMOTE_ROOT = "/home/ubuntu/bucky-vpn-integration"
REMOTE_SERVER_SN_PORT = 3624
REMOTE_SERVER_HTTP_PORT = 3445
REMOTE_CLIENT_API_PORT = 4536
REMOTE_CLIENT_P2P_PORT = 3624
DEFAULT_MULTIPASS_IMAGE = "24.04"
DEFAULT_BASE_INSTANCE_NAME = "bucky-vpn-integration-base-24-04"
DEFAULT_PARALLEL_INSTANCES = 2
DEFAULT_COMMAND_TIMEOUT_SEC = 1800
WINDOWS_MULTIPASS = Path("/mnt/c/Program Files/Multipass/bin/multipass.exe")
MULTIPASS_COMMAND: list[str] | None = None
MULTIPASS_CLONE_LOCK = threading.Lock()


class IntegrationError(RuntimeError):
    pass


@dataclass(frozen=True)
class ServerSpec:
    name: str
    sn_enabled: bool
    pn_enabled: bool
    control_server: str | None = None
    cpus: int | None = None


@dataclass(frozen=True)
class ClientSpec:
    name: str
    networks: tuple[str, ...]


@dataclass
class ClientRuntime:
    spec: ClientSpec
    index: int
    data_dir: str
    env: dict[str, str]
    base_url: str
    process: "RemoteProcess"
    node_id: str | None = None


@dataclass(frozen=True)
class Scenario:
    name: str
    servers: tuple[ServerSpec, ...]
    clients: tuple[ClientSpec, ...]


SCENARIOS = (
    Scenario(
        name="two-clients-combined-control-and-proxy",
        servers=(ServerSpec("control-pn", sn_enabled=True, pn_enabled=True),),
        clients=(
            ClientSpec("client-a", ("mesh-a",)),
            ClientSpec("client-b", ("mesh-a",)),
        ),
    ),
    Scenario(
        name="two-clients-separate-control-and-proxy",
        servers=(
            ServerSpec("control", sn_enabled=True, pn_enabled=False),
            ServerSpec("proxy", sn_enabled=False, pn_enabled=True, control_server="control"),
        ),
        clients=(
            ClientSpec("client-a", ("mesh-a",)),
            ClientSpec("client-b", ("mesh-a",)),
        ),
    ),
    Scenario(
        name="two-clients-one-cpu-control-and-proxy",
        servers=(
            ServerSpec("control", sn_enabled=True, pn_enabled=False, cpus=1),
            ServerSpec(
                "proxy",
                sn_enabled=False,
                pn_enabled=True,
                control_server="control",
                cpus=1,
            ),
        ),
        clients=(
            ClientSpec("client-a", ("mesh-a",)),
            ClientSpec("client-b", ("mesh-a",)),
        ),
    ),
    Scenario(
        name="three-clients-two-proxies-three-pairwise-networks",
        servers=(
            ServerSpec("control", sn_enabled=True, pn_enabled=False),
            ServerSpec("proxy-one", sn_enabled=False, pn_enabled=True, control_server="control"),
            ServerSpec("proxy-two", sn_enabled=False, pn_enabled=True, control_server="control"),
        ),
        clients=(
            ClientSpec("client-a", ("mesh-ab", "mesh-ac")),
            ClientSpec("client-b", ("mesh-ab", "mesh-bc")),
            ClientSpec("client-c", ("mesh-ac", "mesh-bc")),
        ),
    ),
)

BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def base58_encode(data: bytes) -> str:
    number = int.from_bytes(data, byteorder="big")
    encoded = ""
    while number > 0:
        number, remainder = divmod(number, 58)
        encoded = BASE58_ALPHABET[remainder] + encoded
    leading_zeroes = len(data) - len(data.lstrip(b"\0"))
    return "1" * leading_zeroes + (encoded or "")


def sha256_join(parts: list[bytes]) -> bytes:
    digest = hashlib.sha256()
    for part in parts:
        digest.update(part)
    return digest.digest()


def login_password(user_name: str, password: str, timestamp: int) -> str:
    stored_password = base58_encode(sha256_join([user_name.encode(), password.encode()]))
    return base58_encode(sha256_join([stored_password.encode(), str(timestamp).encode()]))


def run_host(command: list[str], timeout_sec: int, capture: bool = False) -> subprocess.CompletedProcess[str]:
    printable = " ".join(command)
    print(f"RUN {printable}")
    return subprocess.run(
        command,
        cwd=REPO_ROOT,
        timeout=timeout_sec,
        check=False,
        capture_output=capture,
        text=True,
    )


def checked_host(command: list[str], timeout_sec: int, capture: bool = False) -> subprocess.CompletedProcess[str]:
    try:
        result = run_host(command, timeout_sec, capture)
    except subprocess.TimeoutExpired as exc:
        raise IntegrationError(
            f"command timed out after {timeout_sec}s: {' '.join(command)}"
        ) from exc
    if result.returncode != 0:
        details = ""
        if capture:
            details = f"\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        raise IntegrationError(f"command failed ({result.returncode}): {' '.join(command)}{details}")
    return result


def clone_multipass_instance(base_instance: str, name: str, attempts: int = 3) -> None:
    last_result: subprocess.CompletedProcess[str] | None = None
    for attempt in range(1, attempts + 1):
        if multipass_instance_exists(name):
            print(f"multipass: clone target {name} already exists")
            return

        with MULTIPASS_CLONE_LOCK:
            try:
                result = run_host(
                    [*multipass_command(), "clone", base_instance, "--name", name],
                    timeout_sec=300,
                    capture=True,
                )
            except subprocess.TimeoutExpired as exc:
                if attempt == attempts:
                    raise IntegrationError(
                        f"command timed out after 300s: {' '.join([*multipass_command(), 'clone', base_instance, '--name', name])}"
                    ) from exc
                time.sleep(min(2 * attempt, 8))
                continue

        if result.returncode == 0:
            return
        if multipass_instance_exists(name):
            print(
                f"multipass: clone command exited {result.returncode}, but target {name} exists; continuing"
            )
            return

        last_result = result
        if attempt < attempts and result.returncode in {-13, 1, 2}:
            print(
                f"multipass: clone {base_instance} -> {name} failed with {result.returncode}; "
                f"retrying {attempt + 1}/{attempts}"
            )
            time.sleep(min(2 * attempt, 8))
            continue
        break

    details = ""
    if last_result is not None:
        details = f"\nstdout:\n{last_result.stdout}\nstderr:\n{last_result.stderr}"
    raise IntegrationError(
        f"command failed ({last_result.returncode if last_result else 'unknown'}): "
        f"{' '.join([*multipass_command(), 'clone', base_instance, '--name', name])}{details}"
    )


def shell_join(command: list[str]) -> str:
    return " ".join(quote(part) for part in command)


def env_flag(default: bool, *names: str) -> bool:
    for name in names:
        value = os.environ.get(name)
        if value is None:
            continue
        return value.strip().lower() in {"1", "true", "yes", "on"}
    return default


def env_int(default: int, *names: str) -> int:
    for name in names:
        value = os.environ.get(name)
        if value is None:
            continue
        try:
            return int(value)
        except ValueError as exc:
            raise IntegrationError(f"{name} must be an integer, got {value!r}") from exc
    return default


def multipass_command() -> list[str]:
    return MULTIPASS_COMMAND or ["multipass"]


def is_windows_multipass() -> bool:
    return any(part.lower().endswith("multipass.exe") for part in multipass_command())


def host_path(path: Path) -> str:
    if not is_windows_multipass():
        return str(path)
    result = checked_host(["wslpath", "-w", str(path)], timeout_sec=10, capture=True)
    return result.stdout.strip()


def ensure_multipass_available() -> None:
    global MULTIPASS_COMMAND
    candidates: list[list[str]] = []
    if os.environ.get("MULTIPASS_BIN"):
        candidates.append(split(os.environ["MULTIPASS_BIN"]))
    if WINDOWS_MULTIPASS.exists():
        candidates.append([str(WINDOWS_MULTIPASS)])
    candidates.append(["multipass"])

    failures: list[str] = []
    for candidate in candidates:
        result = run_host([*candidate, "version"], timeout_sec=30, capture=True)
        if result.returncode == 0:
            MULTIPASS_COMMAND = candidate
            print(f"multipass: using {' '.join(candidate)}")
            return
        failures.append(
            f"{' '.join(candidate)} exited {result.returncode}\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )

    raise IntegrationError(
        "bucky-vpn integration requires a working Multipass installation because each "
        "test node runs in its own instance.\n" + "\n\n".join(failures)
    )


def safe_instance_name(*parts: str) -> str:
    raw = "-".join(parts).lower()
    normalized = re.sub(r"[^a-z0-9-]+", "-", raw).strip("-")
    digest = hashlib.sha256(raw.encode("utf-8")).hexdigest()[:8]
    prefix = normalized[: 63 - len(digest) - 1].strip("-")
    return f"{prefix}-{digest}"


def multipass_supports_clone() -> bool:
    result = run_host([*multipass_command(), "help", "clone"], timeout_sec=30, capture=True)
    return result.returncode == 0


def multipass_instance_info(name: str, timeout_sec: int = 120) -> dict[str, Any] | None:
    try:
        result = run_host(
            [*multipass_command(), "info", name, "--format", "json"],
            timeout_sec=timeout_sec,
            capture=True,
        )
    except subprocess.TimeoutExpired:
        return None
    if result.returncode != 0:
        return None
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError:
        return None
    info = payload.get("info", {}).get(name)
    return info if isinstance(info, dict) else None


def multipass_instance_exists(name: str) -> bool:
    return multipass_instance_info(name) is not None


def wait_instance_ip(name: str, timeout_sec: int = 90) -> str:
    deadline = time.monotonic() + timeout_sec
    last_error = ""
    while time.monotonic() < deadline:
        result = run_host(
            [*multipass_command(), "info", name, "--format", "json"],
            20,
            capture=True,
        )
        if result.returncode == 0:
            try:
                info = json.loads(result.stdout)
                entries = info.get("info", {}).get(name, {})
                for ip in entries.get("ipv4", []):
                    if re.match(r"^\d+\.\d+\.\d+\.\d+$", ip):
                        return ip
            except json.JSONDecodeError as exc:
                last_error = str(exc)
        else:
            last_error = result.stderr
        time.sleep(1)
    raise IntegrationError(f"timed out waiting for IPv4 address for {name}: {last_error}")


def stop_instance_if_needed(name: str) -> None:
    info = multipass_instance_info(name)
    if info is None:
        return
    state = str(info.get("state") or info.get("status") or "").lower()
    if "stop" in state:
        return
    checked_host([*multipass_command(), "stop", name], timeout_sec=120)


def prepare_base_instance(base_name: str, image: str) -> None:
    if not multipass_supports_clone():
        raise IntegrationError("current Multipass does not support clone; cannot prepare base instance")

    if not multipass_instance_exists(base_name):
        print(f"multipass: creating base instance {base_name} from {image}")
        checked_host([*multipass_command(), "launch", image, "--name", base_name], timeout_sec=300)
        wait_instance_ip(base_name)
        checked_host(
            [
                *multipass_command(),
                "exec",
                base_name,
                "--",
                "bash",
                "-lc",
                "python3 --version >/dev/null && sudo iptables --version >/dev/null",
            ],
            timeout_sec=60,
        )
    else:
        print(f"multipass: using existing base instance {base_name}")

    stop_instance_if_needed(base_name)


def set_multipass_instance_cpus(name: str, cpus: int) -> None:
    if cpus < 1:
        raise IntegrationError(f"Multipass instance {name} cpus must be >= 1, got {cpus}")
    checked_host(
        [*multipass_command(), "set", f"local.{name}.cpus={cpus}"],
        timeout_sec=120,
        capture=True,
    )


def cleanup_multipass_instance(name: str, keep: bool) -> None:
    if keep:
        print(f"multipass: keeping {name}")
        return
    try:
        run_host([*multipass_command(), "delete", "--purge", name], timeout_sec=120, capture=True)
    except subprocess.TimeoutExpired:
        print(f"multipass: timed out deleting {name}")


class MultipassInstance:
    def __init__(
        self,
        name: str,
        keep: bool,
        base_instance: str | None = None,
        cpus: int | None = None,
    ) -> None:
        self.name = name
        self.keep = keep
        self.ip: str | None = None
        if base_instance is not None:
            clone_multipass_instance(base_instance, name)
            if cpus is not None:
                stop_instance_if_needed(name)
                set_multipass_instance_cpus(name, cpus)
            checked_host([*multipass_command(), "start", name], timeout_sec=420, capture=True)
        else:
            image = os.environ.get("MULTIPASS_IMAGE", DEFAULT_MULTIPASS_IMAGE)
            command = [*multipass_command(), "launch", image, "--name", name]
            if cpus is not None:
                if cpus < 1:
                    raise IntegrationError(
                        f"Multipass instance {name} cpus must be >= 1, got {cpus}"
                    )
                command.extend(["--cpus", str(cpus)])
            checked_host(command, timeout_sec=420, capture=True)
        self.ip = self.wait_ip()
        self.exec(["mkdir", "-p", REMOTE_ROOT], timeout_sec=30)

    def exec(
        self,
        command: list[str],
        timeout_sec: int,
        capture: bool = False,
        sudo: bool = False,
    ) -> subprocess.CompletedProcess[str]:
        remote_command = command if not sudo else ["sudo", *command]
        return checked_host(
            [*multipass_command(), "exec", self.name, "--", *remote_command],
            timeout_sec=timeout_sec,
            capture=capture,
        )

    def stop(self) -> None:
        cleanup_multipass_instance(self.name, self.keep)

    def transfer_to(self, source: Path, target: str, timeout_sec: int = 120) -> None:
        checked_host(
            [*multipass_command(), "transfer", host_path(source), f"{self.name}:{target}"],
            timeout_sec,
        )

    def transfer_from(self, source: str, target: Path, timeout_sec: int = 120) -> None:
        checked_host(
            [*multipass_command(), "transfer", f"{self.name}:{source}", host_path(target)],
            timeout_sec,
        )

    def wait_ip(self, timeout_sec: int = 90) -> str:
        return wait_instance_ip(self.name, timeout_sec)


class RemoteProcess:
    def __init__(
        self,
        name: str,
        instance: MultipassInstance,
        command: list[str],
        env: dict[str, str],
        workdir: str,
        log_path: str,
        pid_path: str,
    ) -> None:
        self.name = name
        self.instance = instance
        self.log_path = log_path
        self.pid_path = pid_path
        self.command = command
        exports = " ".join(f"{key}={quote(value)}" for key, value in sorted(env.items()))
        process_pattern = "^" + re.escape(" ".join(command))
        start_script = (
            f"cd {quote(workdir)} && "
            f"{exports} setsid -f {shell_join(command)} > {quote(log_path)} 2>&1 < /dev/null; "
            "sleep 0.5; "
            f"pgrep -n -f {quote(process_pattern)} > {quote(pid_path)}"
        )
        script = f"sudo -E bash -lc {quote(start_script)}"
        instance.exec(["bash", "-lc", script], timeout_sec=120)

    def poll(self) -> int | None:
        script = (
            f"pid=$(cat {quote(self.pid_path)} 2>/dev/null) || exit 2; "
            "if sudo kill -0 \"$pid\" 2>/dev/null; then exit 0; fi; "
            "exit 1"
        )
        result = run_host(
            [*multipass_command(), "exec", self.instance.name, "--", "bash", "-lc", script],
            timeout_sec=10,
            capture=True,
        )
        if result.returncode == 0:
            return None
        if result.returncode == 2:
            return 2
        return result.returncode

    def stop(self) -> None:
        script = (
            f"pid=$(cat {quote(self.pid_path)} 2>/dev/null) || exit 0; "
            "sudo kill -TERM \"$pid\" 2>/dev/null || true; "
            "sleep 2; "
            "sudo kill -KILL \"$pid\" 2>/dev/null || true; "
            f"rm -f {quote(self.pid_path)}"
        )
        run_host(
            [*multipass_command(), "exec", self.instance.name, "--", "bash", "-lc", script],
            timeout_sec=20,
            capture=True,
        )


def run(command: list[str], timeout_sec: int) -> None:
    checked_host(command, timeout_sec)


def binary_path(name: str) -> Path:
    suffix = ".exe" if os.name == "nt" else ""
    return REPO_ROOT / "target" / "debug" / f"{name}{suffix}"


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def read_log_tail(path: Path, line_count: int = 80) -> str:
    if not path.exists():
        return f"{path}: <missing>"
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    tail = "\n".join(lines[-line_count:])
    return f"{path}:\n{tail}"


def remote_log_tail(process: RemoteProcess, line_count: int = 80) -> str:
    result = run_host(
        [
            *multipass_command(),
            "exec",
            process.instance.name,
            "--",
            "sudo",
            "tail",
            "-n",
            str(line_count),
            process.log_path,
        ],
        timeout_sec=20,
        capture=True,
    )
    if result.returncode != 0:
        return f"{process.instance.name}:{process.log_path}: <unreadable>\n{result.stderr}"
    return f"{process.instance.name}:{process.log_path}:\n{result.stdout}"


REMOTE_LOG_MISSING_NEEDLES = r"""
import json
import sys

path, needles_text = sys.argv[1:3]
needles = json.loads(needles_text)
try:
    with open(path, "r", encoding="utf-8", errors="replace") as handle:
        text = handle.read()
except OSError as exc:
    print(str(exc), file=sys.stderr)
    raise SystemExit(2)
print(json.dumps([needle for needle in needles if needle not in text]))
"""


REMOTE_LOG_ERROR_LINES = r"""
import json
import sys

path, include_control_refresh_errors_text = sys.argv[1:3]
include_control_refresh_errors = include_control_refresh_errors_text == "1"
failures = []
try:
    with open(path, "r", encoding="utf-8", errors="replace") as handle:
        for line in handle:
            stripped = line.rstrip("\n")
            if ("connect pn server" in stripped and "failed" in stripped) or (
                include_control_refresh_errors and "run_proc failed" in stripped
            ):
                failures.append(stripped)
                if len(failures) >= 50:
                    break
except OSError as exc:
    print(str(exc), file=sys.stderr)
    raise SystemExit(2)
print(json.dumps(failures))
"""


def remote_log_missing_needles(process: RemoteProcess, needles: list[str]) -> list[str]:
    result = checked_host(
        [
            *multipass_command(),
            "exec",
            process.instance.name,
            "--",
            "sudo",
            "python3",
            "-c",
            REMOTE_LOG_MISSING_NEEDLES,
            process.log_path,
            json.dumps(needles),
        ],
        timeout_sec=20,
        capture=True,
    )
    return json.loads(result.stdout)


def remote_log_error_lines(
    process: RemoteProcess,
    include_control_refresh_errors: bool = True,
) -> list[str]:
    result = checked_host(
        [
            *multipass_command(),
            "exec",
            process.instance.name,
            "--",
            "sudo",
            "python3",
            "-c",
            REMOTE_LOG_ERROR_LINES,
            process.log_path,
            "1" if include_control_refresh_errors else "0",
        ],
        timeout_sec=20,
        capture=True,
    )
    return json.loads(result.stdout)


def format_process_log_tails(processes: list[RemoteProcess], line_count: int = 80) -> str:
    return "\n\n".join(remote_log_tail(process, line_count) for process in processes)


def install_underlay_block(instance: MultipassInstance, blocked_ips: list[str], reason: str) -> None:
    if not blocked_ips:
        return
    for ip in sorted(set(blocked_ips)):
        print(f"underlay-isolation: {instance.name} blocks {ip} ({reason})")
        instance.exec(
            [
                "sudo",
                "iptables",
                "-w",
                "-I",
                "OUTPUT",
                "1",
                "-d",
                ip,
                "-j",
                "REJECT",
            ],
            timeout_sec=20,
        )
        instance.exec(
            [
                "sudo",
                "iptables",
                "-w",
                "-I",
                "INPUT",
                "1",
                "-s",
                ip,
                "-j",
                "REJECT",
            ],
            timeout_sec=20,
        )


def assert_underlay_unreachable(
    source: MultipassInstance,
    target_ip: str,
    label: str,
) -> None:
    try:
        result = run_host(
            [
                *multipass_command(),
                "exec",
                source.name,
                "--",
                "ping",
                "-c",
                "1",
                "-W",
                "1",
                target_ip,
            ],
            timeout_sec=10,
            capture=True,
        )
    except subprocess.TimeoutExpired:
        return
    if result.returncode == 0:
        raise IntegrationError(
            f"underlay isolation failed: {label} direct ping to {target_ip} succeeded"
        )


def install_client_underlay_isolation(client_instances: dict[str, MultipassInstance]) -> None:
    for client_name, instance in client_instances.items():
        peer_ips = [
            peer.ip
            for peer_name, peer in client_instances.items()
            if peer_name != client_name and peer.ip is not None
        ]
        install_underlay_block(instance, [ip for ip in peer_ips if ip is not None], "client-direct")

    for client_name, instance in client_instances.items():
        for peer_name, peer in client_instances.items():
            if peer_name == client_name or peer.ip is None:
                continue
            assert_underlay_unreachable(
                instance,
                peer.ip,
                f"{client_name}->{peer_name}",
            )


def install_control_underlay_isolation(
    client_instances: dict[str, MultipassInstance],
    server_instances: dict[str, MultipassInstance],
    scenario: Scenario,
) -> None:
    control_ips = [
        server_instances[server.name].ip
        for server in scenario.servers
        if server.sn_enabled and not server.pn_enabled and server_instances[server.name].ip is not None
    ]
    for client_name, instance in client_instances.items():
        install_underlay_block(
            instance,
            [ip for ip in control_ips if ip is not None],
            f"{scenario.name}/{client_name} non-pn-control",
        )
        for ip in control_ips:
            if ip is not None:
                assert_underlay_unreachable(
                    instance,
                    ip,
                    f"{scenario.name}/{client_name}->non-pn-control",
                )


def wait_client_virtual_ping(
    source: ClientRuntime,
    target_ip: str,
    label: str,
    diagnostic_processes: list[RemoteProcess] | None = None,
    timeout_sec: int = 60,
) -> None:
    deadline = time.monotonic() + timeout_sec
    last_stdout = ""
    last_stderr = ""
    while time.monotonic() < deadline:
        result = run_host(
            [
                *multipass_command(),
                "exec",
                source.process.instance.name,
                "--",
                "ping",
                "-c",
                "3",
                "-W",
                "2",
                target_ip,
            ],
            timeout_sec=15,
            capture=True,
        )
        last_stdout = result.stdout
        last_stderr = result.stderr
        if result.returncode == 0:
            return
        if source.process.poll() is not None:
            break
        time.sleep(1)

    processes = diagnostic_processes or [source.process]
    seen: set[tuple[str, str]] = set()
    unique_processes: list[RemoteProcess] = []
    for process in processes:
        key = (process.instance.name, process.log_path)
        if key not in seen:
            seen.add(key)
            unique_processes.append(process)

    raise IntegrationError(
        f"virtual data-plane ping failed for {label} target {target_ip}\n"
        f"stdout:\n{last_stdout}\nstderr:\n{last_stderr}\n"
        f"{format_process_log_tails(unique_processes)}"
    )


def assert_client_data_plane_via_pn(
    clients: list[ClientRuntime],
    network_ip_by_client: dict[str, dict[str, str]],
    server_processes: list[RemoteProcess],
) -> None:
    client_by_name = {client.spec.name: client for client in clients}
    diagnostic_processes = server_processes + [client.process for client in clients]
    for network_name, ip_by_client in sorted(network_ip_by_client.items()):
        for source_name, source_ip in sorted(ip_by_client.items()):
            source = client_by_name[source_name]
            for target_name, target_ip in sorted(ip_by_client.items()):
                if target_name == source_name:
                    continue
                label = f"{network_name}:{source_name}({source_ip})->{target_name}({target_ip})"
                wait_client_virtual_ping(source, target_ip, label, diagnostic_processes)


def traffic_u64(item: dict[str, Any], field: str) -> int:
    if not isinstance(item, dict):
        raise IntegrationError(f"traffic item is not an object: {item!r}")
    value = item.get(field, 0)
    try:
        parsed = int(str(value))
    except (TypeError, ValueError) as exc:
        raise IntegrationError(f"traffic field {field} is not an integer: {value!r}") from exc
    if parsed < 0:
        raise IntegrationError(f"traffic field {field} is negative: {value!r}")
    return parsed


def traffic_total(item: dict[str, Any]) -> int:
    return traffic_u64(item, "tx_bytes") + traffic_u64(item, "rx_bytes")


def wait_pn_traffic_reported(
    control: dict[str, Any],
    network_by_name: dict[str, Any],
    clients: list[ClientRuntime],
    timeout_sec: int = 90,
) -> None:
    expected_by_network: dict[str, set[str]] = {}
    for client in clients:
        if client.node_id is None:
            raise IntegrationError(f"{client.spec.name} has no joined node id")
        for network_name in client.spec.networks:
            expected_by_network.setdefault(network_name, set()).add(client.node_id)

    deadline = time.monotonic() + timeout_sec
    last_members: dict[str, Any] = {}
    last_user_stats: dict[str, Any] = {}
    while time.monotonic() < deadline:
        all_members_reported = True
        for network_name, expected_node_ids in expected_by_network.items():
            members = http_json_remote(
                control["instance"],
                control["base_url"],
                "POST",
                "/get_network_member",
                {"network_id": str(network_by_name[network_name]["id"])},
                token=control["token"],
            )
            if not isinstance(members, list):
                raise IntegrationError(f"/get_network_member returned non-list: {members!r}")
            last_members[network_name] = members
            if not all(isinstance(member, dict) for member in members):
                raise IntegrationError(f"/get_network_member returned invalid members: {members!r}")
            member_by_id = {str(member.get("id")): member for member in members}
            for node_id in expected_node_ids:
                member = member_by_id.get(node_id)
                if member is None or traffic_total(member) == 0:
                    all_members_reported = False
                    break
            if not all_members_reported:
                break

        user_stats = http_json_remote(
            control["instance"],
            control["base_url"],
            "GET",
            "/get_user_traffic_stats",
            token=control["token"],
        )
        last_user_stats = user_stats
        user_stats_reported = (
            traffic_u64(user_stats, "tx_bytes") > 0
            and traffic_u64(user_stats, "rx_bytes") > 0
        )
        if all_members_reported and user_stats_reported:
            print(
                "traffic-report: control API reports non-zero member and user traffic "
                f"stats: {json.dumps(user_stats, sort_keys=True)}"
            )
            return
        time.sleep(1)

    raise IntegrationError(
        "timed out waiting for PN traffic report stats on control API\n"
        f"user_stats={json.dumps(last_user_stats, sort_keys=True)}\n"
        f"members={json.dumps(last_members, sort_keys=True)}"
    )


def assert_parallel_client_daemons_supported(required_clients: int) -> None:
    main_rs = read_text(CLIENT_MAIN)
    cli_rs = read_text(CLIENT_CLI)
    hardcoded_daemon_port = (
        f'HttpServerConfig::new("127.0.0.1", {DEFAULT_CLIENT_HTTP_PORT})' in main_rs
    )
    hardcoded_cli_target = f"http://127.0.0.1:{DEFAULT_CLIENT_HTTP_PORT}" in cli_rs
    configurable_port = (
        "api.port" in main_rs
        or "api.port" in cli_rs
        or "VPN_API_PORT" in main_rs
        or "VPN_API_PORT" in cli_rs
        or "client.http.port" in main_rs
        or "client.http.port" in cli_rs
    )
    if required_clients > 1 and (hardcoded_daemon_port or hardcoded_cli_target) and not configurable_port:
        raise IntegrationError(
            "Multipass PN proxy integration still requires every vpn-client daemon "
            f"to expose a configurable local HTTP API, but bucky-vpn currently binds to fixed "
            f"127.0.0.1:{DEFAULT_CLIENT_HTTP_PORT} and the join CLI posts to that same "
            "fixed address. Add a configurable client HTTP listen/target port before "
            "this script can run the requested two-client and three-client topologies."
        )


def wait_tcp(host: str, port: int, timeout_sec: int, process: RemoteProcess | None = None) -> None:
    deadline = time.monotonic() + timeout_sec
    last_error: OSError | None = None
    while time.monotonic() < deadline:
        if process is not None and process.poll() is not None:
            raise IntegrationError(
                f"{process.name} exited before {host}:{port} became ready\n"
                f"{remote_log_tail(process)}"
            )
        try:
            with socket.create_connection((host, port), timeout=1):
                return
        except OSError as exc:
            last_error = exc
            time.sleep(0.2)
    raise IntegrationError(f"timed out waiting for {host}:{port}: {last_error}")


def wait_remote_tcp(
    instance: MultipassInstance,
    host: str,
    port: int,
    timeout_sec: int,
    process: RemoteProcess | None = None,
) -> None:
    deadline = time.monotonic() + timeout_sec
    last_error = ""
    script = (
        "import socket, sys; "
        "host=sys.argv[1]; port=int(sys.argv[2]); "
        "s=socket.create_connection((host, port), timeout=1); s.close()"
    )
    while time.monotonic() < deadline:
        if process is not None and process.poll() is not None:
            raise IntegrationError(
                f"{process.name} exited before {host}:{port} became ready\n"
                f"{remote_log_tail(process)}"
            )
        result = run_host(
            [
                *multipass_command(),
                "exec",
                instance.name,
                "--",
                "python3",
                "-c",
                script,
                host,
                str(port),
            ],
            timeout_sec=5,
            capture=True,
        )
        if result.returncode == 0:
            return
        last_error = result.stderr or result.stdout
        time.sleep(0.2)
    raise IntegrationError(f"timed out waiting for {instance.name}:{host}:{port}: {last_error}")


REMOTE_HTTP_CLIENT = r"""
import json
import sys
import urllib.error
import urllib.request

url, method, token, body_text, timeout_text = sys.argv[1:6]
data = None if body_text == "" else body_text.encode("utf-8")
headers = {"Content-Type": "application/json"}
if token:
    headers["Authorization"] = f"Bearer {token}"
req = urllib.request.Request(url, data=data, headers=headers, method=method)
try:
    with urllib.request.urlopen(req, timeout=int(timeout_text)) as resp:
        payload = json.loads(resp.read().decode("utf-8"))
except (TimeoutError, urllib.error.URLError) as exc:
    print(f"{method} {url} failed: {exc}", file=sys.stderr)
    raise SystemExit(2)
if payload.get("err") != 0:
    print(
        f"{method} {url} returned err={payload.get('err')} msg={payload.get('msg')}",
        file=sys.stderr,
    )
    raise SystemExit(3)
print(json.dumps(payload.get("result")))
"""


def http_json_remote(
    instance: MultipassInstance,
    base_url: str,
    method: str,
    path: str,
    body: dict[str, Any] | None = None,
    token: str | None = None,
    timeout_sec: int = 10,
) -> Any:
    body_text = "" if body is None else json.dumps(body)
    result = checked_host(
        [
            *multipass_command(),
            "exec",
            instance.name,
            "--",
            "python3",
            "-c",
            REMOTE_HTTP_CLIENT,
            f"{base_url}{path}",
            method,
            token or "",
            body_text,
            str(timeout_sec),
        ],
        timeout_sec=timeout_sec + 10,
        capture=True,
    )
    text = result.stdout.strip()
    return None if text == "" else json.loads(text)


def http_json_remote_retry(
    instance: MultipassInstance,
    base_url: str,
    method: str,
    path: str,
    body: dict[str, Any] | None = None,
    token: str | None = None,
    timeout_sec: int = 10,
    attempts: int = 3,
) -> Any:
    last_error: IntegrationError | None = None
    for attempt in range(1, attempts + 1):
        try:
            return http_json_remote(instance, base_url, method, path, body, token, timeout_sec)
        except IntegrationError as exc:
            last_error = exc
            if attempt == attempts:
                break
            time.sleep(min(2 * attempt, 5))
    raise last_error or IntegrationError(f"{method} {base_url}{path} failed")


def http_json(
    base_url: str,
    method: str,
    path: str,
    body: dict[str, Any] | None = None,
    token: str | None = None,
    timeout_sec: int = 10,
) -> Any:
    data = None if body is None else json.dumps(body).encode("utf-8")
    headers = {"Content-Type": "application/json"}
    if token is not None:
        headers["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(
        f"{base_url}{path}",
        data=data,
        headers=headers,
        method=method,
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout_sec) as resp:
            payload = json.loads(resp.read().decode("utf-8"))
    except (TimeoutError, urllib.error.URLError) as exc:
        raise IntegrationError(f"{method} {base_url}{path} failed: {exc}") from exc
    if payload.get("err") != 0:
        raise IntegrationError(
            f"{method} {base_url}{path} returned err={payload.get('err')} msg={payload.get('msg')}"
        )
    return payload.get("result")


def http_json_retry(
    base_url: str,
    method: str,
    path: str,
    body: dict[str, Any] | None = None,
    token: str | None = None,
    timeout_sec: int = 10,
    attempts: int = 3,
) -> Any:
    last_error: IntegrationError | None = None
    for attempt in range(1, attempts + 1):
        try:
            return http_json(base_url, method, path, body, token, timeout_sec)
        except IntegrationError as exc:
            last_error = exc
            if attempt == attempts:
                break
            time.sleep(min(2 * attempt, 5))
    raise last_error or IntegrationError(f"{method} {base_url}{path} failed")


def write_server_config(
    root: Path,
    spec: ServerSpec,
    sn_port: int,
    http_port: int,
    control: dict[str, Any] | None,
    node_ip: str,
    remote_data_dir: str,
) -> Path:
    config_dir = root / spec.name
    config_dir.mkdir(parents=True, exist_ok=True)
    lines = [
        f'ip: "{node_ip}"',
        f"port: {sn_port}",
        "http:",
        '  ip: "0.0.0.0"',
        f"  port: {http_port}",
        "sn:",
        f"  enabled: {'true' if spec.sn_enabled else 'false'}",
        "pn:",
        f"  enabled: {'true' if spec.pn_enabled else 'false'}",
        "  report_interval_secs: 1",
    ]
    if control is not None:
        lines.extend(
            [
                "  control_server:",
                f'    id: "{control["id"]}"',
                f'    endpoint: "{control["sn_ip"]}:{control["sn_port"]}"',
            ]
        )
    lines.extend(
        [
            "admin:",
            '  name: "admin"',
            '  password: "admin"',
            'jwt:',
            '  key: "process-integration-test-key"',
            "data:",
            f'  dir: "{remote_data_dir}"',
            "log: true",
            'log.level: "debug"',
        ]
    )
    config_path = config_dir / "config.yaml"
    config_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return config_path


def copy_remote_sqlite_db(
    instance: MultipassInstance,
    remote_data_dir: str,
    local_root: Path,
    name: str,
) -> Path:
    archive = f"/tmp/{name}-vpn-db.tgz"
    script = (
        f"cd {quote(remote_data_dir)} && "
        f"sudo tar -czf {quote(archive)} vpn.db vpn.db-wal vpn.db-shm 2>/dev/null "
        f"|| sudo tar -czf {quote(archive)} vpn.db"
    )
    instance.exec(["bash", "-lc", script], timeout_sec=20)
    local_archive = local_root / f"{name}-vpn-db.tgz"
    local_db_dir = local_root / f"{name}-db"
    local_db_dir.mkdir(parents=True, exist_ok=True)
    instance.transfer_from(archive, local_archive)
    checked_host(["tar", "-xzf", str(local_archive), "-C", str(local_db_dir)], timeout_sec=20)
    return local_db_dir / "vpn.db"


REMOTE_SQLITE_QUERY = r"""
import json
import sqlite3
import sys

db_path, sql, params_text = sys.argv[1:4]
params = json.loads(params_text)
try:
    with sqlite3.connect(db_path) as conn:
        rows = conn.execute(sql, params).fetchall()
except sqlite3.Error as exc:
    print(str(exc), file=sys.stderr)
    raise SystemExit(2)
print(json.dumps(rows))
"""


def remote_sqlite_query(
    instance: MultipassInstance,
    remote_data_dir: str,
    sql: str,
    params: list[Any],
) -> list[list[Any]]:
    result = checked_host(
        [
            *multipass_command(),
            "exec",
            instance.name,
            "--",
            "sudo",
            "python3",
            "-c",
            REMOTE_SQLITE_QUERY,
            f"{remote_data_dir}/vpn.db",
            sql,
            json.dumps(params),
        ],
        timeout_sec=20,
        capture=True,
    )
    return json.loads(result.stdout)


def read_admin_server_id(
    instance: MultipassInstance,
    remote_data_dir: str,
    local_root: Path,
    name: str,
    timeout_sec: int = 10,
) -> str:
    deadline = time.monotonic() + timeout_sec
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            rows = remote_sqlite_query(
                instance,
                remote_data_dir,
                "SELECT server_id FROM user WHERE id = ?",
                ["admin"],
            )
            if rows and rows[0] and rows[0][0]:
                return str(rows[0][0])
        except (sqlite3.Error, IntegrationError) as exc:
            last_error = exc
        time.sleep(0.2)
    raise IntegrationError(f"failed to read admin server_id from {instance.name}:{remote_data_dir}: {last_error}")


def read_joined_node_ids_by_name(
    instance: MultipassInstance,
    remote_data_dir: str,
    local_root: Path,
    name: str,
    expected_names: set[str],
    timeout_sec: int = 20,
) -> dict[str, str]:
    deadline = time.monotonic() + timeout_sec
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            ordered_names = sorted(expected_names)
            rows = remote_sqlite_query(
                instance,
                remote_data_dir,
                "SELECT name, node_id FROM joined_node WHERE name IN ({})".format(
                    ",".join("?" for _ in ordered_names)
                ),
                ordered_names,
            )
            found = {str(name): str(node_id) for name, node_id in rows}
            if expected_names.issubset(found.keys()):
                return found
        except (sqlite3.Error, IntegrationError) as exc:
            last_error = exc
        time.sleep(0.2)
    missing = ", ".join(sorted(expected_names))
    raise IntegrationError(
        f"failed to read joined node ids for {missing} from {instance.name}:{remote_data_dir}: {last_error}"
    )


def start_server(
    root: Path,
    server_bin: Path,
    spec: ServerSpec,
    control: dict[str, Any] | None,
    instance: MultipassInstance,
) -> tuple[RemoteProcess, dict[str, Any]]:
    if instance.ip is None:
        raise IntegrationError(f"{instance.name} has no IPv4 address")
    sn_port = REMOTE_SERVER_SN_PORT
    http_port = REMOTE_SERVER_HTTP_PORT
    remote_node_root = f"{REMOTE_ROOT}/{spec.name}"
    remote_data_dir = f"{remote_node_root}/data"
    config_path = write_server_config(
        root,
        spec,
        sn_port,
        http_port,
        control,
        instance.ip,
        remote_data_dir,
    )
    remote_config_path = f"{remote_node_root}/config.yaml"
    remote_log_path = f"{remote_node_root}/process.log"
    remote_pid_path = f"{remote_node_root}/process.pid"
    instance.exec(["mkdir", "-p", remote_node_root, remote_data_dir], timeout_sec=30)
    instance.transfer_to(server_bin, f"{remote_node_root}/bucky-vpn-server")
    instance.transfer_to(config_path, remote_config_path)
    instance.exec(["chmod", "+x", f"{remote_node_root}/bucky-vpn-server"], timeout_sec=30)
    process = RemoteProcess(
        spec.name,
        instance,
        ["./bucky-vpn-server", "-c", remote_config_path],
        {"VPN_LOG": "true", "VPN_LOG_LEVEL": "debug"},
        remote_node_root,
        remote_log_path,
        remote_pid_path,
    )
    timestamp = int(time.time())
    base_url = f"http://127.0.0.1:{http_port}"

    if not spec.sn_enabled:
        time.sleep(1)
        if process.poll() is not None:
            raise IntegrationError(
                f"{spec.name} exited after startup\n{remote_log_tail(process)}"
            )
        return process, {
            "name": spec.name,
            "base_url": base_url,
            "token": None,
            "id": None,
            "sn_ip": instance.ip,
            "sn_port": sn_port,
            "http_port": http_port,
            "data_dir": remote_data_dir,
            "instance": instance,
            "process": process,
            "log_path": process.log_path,
        }

    wait_remote_tcp(instance, "127.0.0.1", http_port, timeout_sec=30, process=process)
    token_result = http_json_remote(
        instance,
        base_url,
        "POST",
        "/account/login",
        {
            "user_name": "admin",
            "password": login_password("admin", "admin", timestamp),
            "timestamp": timestamp,
        },
    )
    server_id = read_admin_server_id(instance, remote_data_dir, root, spec.name)
    return process, {
        "name": spec.name,
        "base_url": base_url,
        "token": token_result["session"],
        "id": server_id,
        "sn_ip": instance.ip,
        "sn_port": sn_port,
        "http_port": http_port,
        "data_dir": remote_data_dir,
        "instance": instance,
        "process": process,
        "log_path": process.log_path,
    }


def start_client(
    root: Path,
    client_bin: Path,
    scenario: Scenario,
    client: ClientSpec,
    index: int,
    instance: MultipassInstance,
    env: dict[str, str] | None = None,
    data_dir: str | None = None,
    log_suffix: str = "process",
) -> ClientRuntime:
    if instance.ip is None:
        raise IntegrationError(f"{instance.name} has no IPv4 address")
    remote_node_root = f"{REMOTE_ROOT}/{scenario.name}/{client.name}"
    data_dir = data_dir or f"{remote_node_root}/data"
    remote_log_path = f"{remote_node_root}/{log_suffix}.log"
    remote_pid_path = f"{remote_node_root}/{log_suffix}.pid"
    instance.exec(["mkdir", "-p", remote_node_root, data_dir], timeout_sec=30)
    instance.transfer_to(client_bin, f"{remote_node_root}/bucky-vpn")
    instance.exec(["chmod", "+x", f"{remote_node_root}/bucky-vpn"], timeout_sec=30)
    if env is None:
        env = {
            "VPN_DATA_DIR": data_dir,
            "VPN_P2P_PORT": str(REMOTE_CLIENT_P2P_PORT),
            "VPN_API_IP": "0.0.0.0",
            "VPN_API_PORT": str(REMOTE_CLIENT_API_PORT),
            "VPN_LOG": "true",
            "VPN_LOG_LEVEL": "debug",
        }
    process = RemoteProcess(
        client.name,
        instance,
        ["./bucky-vpn", "daemon"],
        env,
        remote_node_root,
        remote_log_path,
        remote_pid_path,
    )
    wait_remote_tcp(instance, "127.0.0.1", int(env["VPN_API_PORT"]), timeout_sec=30, process=process)
    return ClientRuntime(
        spec=client,
        index=index,
        data_dir=data_dir,
        env=env,
        base_url=f"http://127.0.0.1:{env['VPN_API_PORT']}",
        process=process,
    )


def assign_joined_clients_to_networks(
    control: dict[str, Any],
    network_by_name: dict[str, Any],
    clients: list[ClientRuntime],
) -> dict[str, dict[str, str]]:
    network_ip_by_client: dict[str, dict[str, str]] = {}
    ordered_networks = sorted(network_by_name)
    network_index_by_name = {name: index for index, name in enumerate(ordered_networks, 1)}
    for client in clients:
        if client.node_id is None:
            raise IntegrationError(f"{client.spec.name} has no joined node id")
        http_json_remote_retry(
            control["instance"],
            control["base_url"],
            "POST",
            "/allow_join",
            {"node_id": client.node_id, "allow_join": True},
            token=control["token"],
            attempts=5,
        )
        for network_name in client.spec.networks:
            network = network_by_name[network_name]
            ip_addr = f"10.{network_index_by_name[network_name]}.0.{client.index + 2}"
            http_json_remote_retry(
                control["instance"],
                control["base_url"],
                "POST",
                "/add_network_member",
                {
                    "network_id": str(network["id"]),
                    "node_id": client.node_id,
                    "ip_addr": ip_addr,
                },
                token=control["token"],
                attempts=5,
            )
            network_ip_by_client.setdefault(network_name, {})[client.spec.name] = ip_addr
    return network_ip_by_client


def wait_and_approve_proxy_nodes(
    control: dict[str, Any],
    expected_count: int,
    timeout_sec: int = 60,
) -> None:
    if expected_count == 0:
        return
    deadline = time.monotonic() + timeout_sec
    last_seen: list[dict[str, Any]] = []
    while time.monotonic() < deadline:
        nodes = http_json_remote(
            control["instance"],
            control["base_url"],
            "GET",
            "/pn_proxy_nodes",
            token=control["token"],
        )
        last_seen = nodes
        live_nodes = [node for node in nodes if node.get("live")]
        if len(live_nodes) >= expected_count:
            for node in live_nodes:
                if node.get("status") != "approved":
                    http_json_remote(
                        control["instance"],
                        control["base_url"],
                        "POST",
                        "/approve_pn_proxy_node",
                        {
                            "pn_server": node["pn_server"],
                            "comment": "multipass integration test",
                        },
                        token=control["token"],
                    )
            return
        time.sleep(1)
    raise IntegrationError(
        f"timed out waiting for live proxy nodes: expected={expected_count} seen={last_seen}"
    )


def wait_network_members_registered(
    control: dict[str, Any],
    network_by_name: dict[str, Any],
    clients: list[ClientRuntime],
    server_processes: list[RemoteProcess],
    timeout_sec: int = 60,
) -> None:
    expected_by_network: dict[str, set[str]] = {}
    for client in clients:
        if client.node_id is None:
            raise IntegrationError(f"{client.spec.name} has no joined node id")
        for network_name in client.spec.networks:
            expected_by_network.setdefault(network_name, set()).add(client.node_id)

    deadline = time.monotonic() + timeout_sec
    last_seen: dict[str, Any] = {}
    while time.monotonic() < deadline:
        all_registered = True
        for network_name, expected_nodes in expected_by_network.items():
            members = http_json_remote(
                control["instance"],
                control["base_url"],
                "POST",
                "/get_network_member",
                {"network_id": str(network_by_name[network_name]["id"])},
                token=control["token"],
            )
            last_seen[network_name] = members
            registered_nodes = {member["id"] for member in members}
            if not expected_nodes.issubset(registered_nodes):
                all_registered = False
                break
        if all_registered:
            return
        time.sleep(1)
    log_paths = ", ".join(str(client.process.log_path) for client in clients)
    process_logs = format_process_log_tails(server_processes + [client.process for client in clients])
    raise IntegrationError(
        f"timed out waiting for registered network members: {last_seen}; client logs: {log_paths}\n"
        f"{process_logs}"
    )


def wait_client_vpn_runtime_ready(
    clients: list[ClientRuntime],
    network_ip_by_client: dict[str, dict[str, str]],
    timeout_sec: int = 120,
) -> None:
    deadline = time.monotonic() + timeout_sec
    missing_by_client: dict[str, list[str]] = {}
    while time.monotonic() < deadline:
        missing_by_client = {}
        for client in clients:
            expected = [
                f"create tun device {network_name} ip {network_ip_by_client[network_name][client.spec.name]}"
                for network_name in client.spec.networks
            ]
            missing = remote_log_missing_needles(client.process, expected)
            if missing:
                missing_by_client[client.spec.name] = missing
        if not missing_by_client:
            return
        time.sleep(1)

    logs = format_process_log_tails([client.process for client in clients])
    raise IntegrationError(
        f"timed out waiting for client VPN runtime readiness: {missing_by_client}\n{logs}"
    )


def assert_client_logs_clean(
    clients: list[ClientRuntime],
    include_control_refresh_errors: bool = True,
) -> None:
    failures: list[str] = []
    for client in clients:
        for line in remote_log_error_lines(client.process, include_control_refresh_errors):
            failures.append(f"{client.process.log_path}: {line}")
    if failures:
        raise IntegrationError("client runtime reported PN setup errors:\n" + "\n".join(failures))


def join_client_networks(
    control: dict[str, Any],
    network_by_name: dict[str, Any],
    scenario: Scenario,
    client: ClientRuntime,
) -> None:
    for network_name in client.spec.networks:
        network = network_by_name[network_name]
        try:
            http_json_remote_retry(
                client.process.instance,
                client.base_url,
                "POST",
                "/join",
                {
                    "server": control["sn_ip"],
                    "server_port": control["sn_port"],
                    "server_id": control["id"],
                    "group_id": int(network["group_id"]),
                    "name": f"{scenario.name}-{client.spec.name}-{client.index}",
                },
                timeout_sec=60,
                attempts=4,
            )
        except IntegrationError as exc:
            processes: list[RemoteProcess] = []
            if isinstance(control.get("process"), RemoteProcess):
                processes.append(control["process"])
            processes.append(client.process)
            raise IntegrationError(
                f"{scenario.name}/{client.spec.name}/{network_name} join failed via "
                f"{control['id']}@{control['sn_ip']}:{control['sn_port']}: {exc}\n"
                f"{format_process_log_tails(processes)}"
            ) from exc


def create_instance(
    run_id: str,
    scenario: Scenario,
    node_name: str,
    keep: bool,
    base_instance: str | None,
    cpus: int | None,
) -> MultipassInstance:
    name = safe_instance_name("bvi", run_id, scenario.name, node_name)
    source = f" from {base_instance}" if base_instance is not None else ""
    cpu_text = f" with {cpus} cpu(s)" if cpus is not None else ""
    print(f"multipass: creating {name} for {scenario.name}/{node_name}{source}{cpu_text}")
    return MultipassInstance(name, keep, base_instance=base_instance, cpus=cpus)


def create_scenario_instances(
    run_id: str,
    scenario: Scenario,
    keep_instances: bool,
    base_instance: str | None,
    parallel_instances: int,
) -> tuple[list[MultipassInstance], dict[str, MultipassInstance], dict[str, MultipassInstance]]:
    nodes: list[tuple[str, str, int | None]] = []
    nodes.extend(("server", spec.name, spec.cpus) for spec in scenario.servers)
    nodes.extend(("client", spec.name, None) for spec in scenario.clients)

    instances: list[MultipassInstance] = []
    server_instances: dict[str, MultipassInstance] = {}
    client_instances: dict[str, MultipassInstance] = {}

    def create(node: tuple[str, str, int | None]) -> tuple[str, str, MultipassInstance]:
        kind, node_name, cpus = node
        instance = create_instance(run_id, scenario, node_name, keep_instances, base_instance, cpus)
        return kind, node_name, instance

    try:
        if parallel_instances <= 1 or len(nodes) <= 1:
            results = []
            for node in nodes:
                result = create(node)
                results.append(result)
                instances.append(result[2])
        else:
            workers = min(parallel_instances, len(nodes))
            print(f"multipass: creating {len(nodes)} instances with parallelism {workers}")
            results = []
            with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as executor:
                futures = [executor.submit(create, node) for node in nodes]
                for future in concurrent.futures.as_completed(futures):
                    kind, node_name, instance = future.result()
                    results.append((kind, node_name, instance))
                    instances.append(instance)

        for kind, node_name, instance in results:
            if instance not in instances:
                instances.append(instance)
            if kind == "server":
                server_instances[node_name] = instance
            else:
                client_instances[node_name] = instance
        return instances, server_instances, client_instances
    except Exception:
        cleaned: set[str] = set()
        for instance in reversed(instances):
            instance.stop()
            cleaned.add(instance.name)
        for _kind, node_name, _cpus in reversed(nodes):
            name = safe_instance_name("bvi", run_id, scenario.name, node_name)
            if name not in cleaned:
                cleanup_multipass_instance(name, keep_instances)
        raise


def run_scenario(
    root: Path,
    client_bin: Path,
    server_bin: Path,
    scenario: Scenario,
    run_id: str,
    keep_instances: bool,
    base_instance: str | None,
    parallel_instances: int,
) -> None:
    required_clients = len(scenario.clients)
    assert_parallel_client_daemons_supported(required_clients)
    processes: list[RemoteProcess] = []
    instances: list[MultipassInstance] = []
    server_infos: dict[str, dict[str, Any]] = {}
    client_runtimes: list[ClientRuntime] = []
    server_instances: dict[str, MultipassInstance] = {}
    client_instances: dict[str, MultipassInstance] = {}
    try:
        instances, server_instances, client_instances = create_scenario_instances(
            run_id,
            scenario,
            keep_instances,
            base_instance,
            parallel_instances,
        )

        install_client_underlay_isolation(client_instances)

        for spec in scenario.servers:
            control = server_infos.get(spec.control_server) if spec.control_server else None
            process, info = start_server(
                root / scenario.name,
                server_bin,
                spec,
                control,
                server_instances[spec.name],
            )
            processes.append(process)
            server_infos[spec.name] = info
        time.sleep(2)

        control = next(info for info in server_infos.values() if info["name"].startswith("control"))
        expected_remote_proxy_count = sum(
            1 for spec in scenario.servers if spec.pn_enabled and spec.control_server is not None
        )
        wait_and_approve_proxy_nodes(control, expected_remote_proxy_count)
        for network_index, network_name in enumerate(sorted({n for c in scenario.clients for n in c.networks}), 1):
            http_json_remote(
                control["instance"],
                control["base_url"],
                "POST",
                "/add_network",
                {"name": network_name, "ip_addr": f"10.{network_index}.0.0", "mask": 24},
                token=control["token"],
            )

        networks = http_json_remote(
            control["instance"],
            control["base_url"],
            "GET",
            "/get_networks",
            token=control["token"],
        )
        network_by_name = {item["name"]: item for item in networks}
        missing_pn = [name for name, item in network_by_name.items() if item.get("pn_server") is None]
        if missing_pn:
            raise IntegrationError(f"networks missing selected PN server: {', '.join(missing_pn)}")

        for index, client in enumerate(scenario.clients):
            runtime = start_client(
                root,
                client_bin,
                scenario,
                client,
                index,
                client_instances[client.name],
            )
            processes.append(runtime.process)
            client_runtimes.append(runtime)
            join_client_networks(control, network_by_name, scenario, runtime)

        expected_join_names = {
            f"{scenario.name}-{client.spec.name}-{client.index}" for client in client_runtimes
        }
        node_ids_by_name = read_joined_node_ids_by_name(
            control["instance"],
            control["data_dir"],
            root / scenario.name,
            "control-joined",
            expected_join_names,
        )
        for client in client_runtimes:
            client.node_id = node_ids_by_name[f"{scenario.name}-{client.spec.name}-{client.index}"]

        network_ip_by_client = assign_joined_clients_to_networks(
            control, network_by_name, client_runtimes
        )

        for client in client_runtimes:
            client.process.stop()
        restarted_clients: list[ClientRuntime] = []
        for client in client_runtimes:
            restarted = start_client(
                root,
                client_bin,
                scenario,
                client.spec,
                client.index,
                client.process.instance,
                env=client.env,
                data_dir=client.data_dir,
                log_suffix="process-restarted",
            )
            restarted.node_id = client.node_id
            processes.append(restarted.process)
            restarted_clients.append(restarted)
        client_runtimes = restarted_clients

        wait_network_members_registered(
            control,
            network_by_name,
            client_runtimes,
            processes[: len(scenario.servers)],
        )
        wait_client_vpn_runtime_ready(client_runtimes, network_ip_by_client)
        assert_client_logs_clean(client_runtimes)
        install_control_underlay_isolation(client_instances, server_instances, scenario)
        assert_client_data_plane_via_pn(
            client_runtimes,
            network_ip_by_client,
            processes[: len(scenario.servers)],
        )
        wait_pn_traffic_reported(control, network_by_name, client_runtimes)
        assert_client_logs_clean(client_runtimes, include_control_refresh_errors=False)
    finally:
        for process in reversed(processes):
            process.stop()
        for instance in reversed(instances):
            instance.stop()


def main(argv: list[str]) -> int:
    explicit_use_base_image = "--use-base-image" in argv
    explicit_no_use_base_image = "--no-use-base-image" in argv
    parser = argparse.ArgumentParser()
    parser.add_argument("--keep-temp", action="store_true")
    parser.add_argument("--no-build", action="store_true")
    parser.add_argument(
        "--prepare-base-image",
        action="store_true",
        default=env_flag(False, "BUCKY_VPN_INTEGRATION_PREPARE_BASE_IMAGE"),
        help="create or reuse the stopped Multipass base instance used for fast clone startup",
    )
    parser.add_argument(
        "--use-base-image",
        action=argparse.BooleanOptionalAction,
        default=env_flag(True, "BUCKY_VPN_INTEGRATION_USE_BASE_IMAGE"),
        help="create test instances by cloning the prepared base instance instead of launching the image",
    )
    parser.add_argument(
        "--base-image-name",
        default=os.environ.get(
            "BUCKY_VPN_INTEGRATION_BASE_INSTANCE",
            os.environ.get("MULTIPASS_BASE_INSTANCE", DEFAULT_BASE_INSTANCE_NAME),
        ),
        help=f"Multipass base instance name for --prepare-base-image/--use-base-image (default: {DEFAULT_BASE_INSTANCE_NAME})",
    )
    parser.add_argument(
        "--parallel-instances",
        type=int,
        default=env_int(
            DEFAULT_PARALLEL_INSTANCES,
            "BUCKY_VPN_INTEGRATION_PARALLEL_INSTANCES",
            "MULTIPASS_PARALLEL_INSTANCES",
        ),
        help=f"number of Multipass instances to create concurrently per scenario (default: {DEFAULT_PARALLEL_INSTANCES})",
    )
    parser.add_argument("--timeout-sec", type=int, default=DEFAULT_COMMAND_TIMEOUT_SEC)
    args = parser.parse_args(argv)

    ensure_multipass_available()

    image = os.environ.get("MULTIPASS_IMAGE", DEFAULT_MULTIPASS_IMAGE)
    if args.prepare_base_image or args.use_base_image:
        prepare_base_instance(args.base_image_name, image)

    if args.prepare_base_image and not explicit_use_base_image and not explicit_no_use_base_image:
        return 0

    if not args.no_build:
        run(["cargo", "build", "-p", "bucky-vpn", "-p", "bucky-vpn-server"], args.timeout_sec)

    client_bin = binary_path("bucky-vpn")
    server_bin = binary_path("bucky-vpn-server")
    for binary in (client_bin, server_bin):
        if not binary.exists():
            raise IntegrationError(f"missing binary {binary}; run cargo build first")

    temp_parent = REPO_ROOT / "test-results" / "tmp"
    temp_parent.mkdir(parents=True, exist_ok=True)
    temp_root = Path(
        tempfile.mkdtemp(prefix="bucky-vpn-process-integration-", dir=temp_parent)
    )
    print(f"process-integration: workdir {temp_root}")
    run_id = str(os.getpid())
    succeeded = False
    try:
        for scenario in SCENARIOS:
            print(f"process-integration: scenario {scenario.name}")
            run_scenario(
                temp_root,
                client_bin,
                server_bin,
                scenario,
                run_id,
                args.keep_temp,
                args.base_image_name if args.use_base_image else None,
                max(args.parallel_instances, 1),
            )
        succeeded = True
    finally:
        if args.keep_temp or not succeeded:
            print(f"process-integration: keeping {temp_root}")
        else:
            shutil.rmtree(temp_root, ignore_errors=True)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except IntegrationError as exc:
        print(f"process-integration: {exc}", file=sys.stderr)
        raise SystemExit(1)
