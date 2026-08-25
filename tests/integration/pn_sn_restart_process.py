#!/usr/bin/env python3
"""Live SN-only restart regression using the repository Multipass harness helpers."""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import re
import shutil
import sys
import tempfile
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
HARNESS = ROOT / "harness/scripts/bucky-vpn-process-integration.py"
RESULT_ROOT = ROOT / ".harness/test-results/pn-sn-restart"
CLIENT_COMMIT_PATTERN = re.compile(
    r"vpn info versions committed: info_version=(\d+), pn_info_version=(\d+)"
)
SN_INITIALIZED_PATTERN = re.compile(
    r"pn assignment version initialized: node_id=([0-9a-z]+), "
    r"pn_info_version=(\d+), network_count=(\d+)"
)
MAX_SN_INITIALIZATION_DELAY_SECONDS = 180
MAX_MULTIPASS_COMMAND_TIMEOUT_SEC = 180
MAX_MULTIPASS_INFO_TIMEOUT_SEC = 30

REMOTE_MATCHING_LOG_LINES = r"""
import json
import re
import sys

path, pattern_text = sys.argv[1:3]
pattern = re.compile(pattern_text)
matches = []
try:
    with open(path, "r", encoding="utf-8", errors="replace") as handle:
        for line in handle:
            stripped = line.rstrip("\n")
            if pattern.search(stripped):
                matches.append(stripped)
except OSError as exc:
    print(str(exc), file=sys.stderr)
    raise SystemExit(2)
print(json.dumps(matches))
"""


def load_harness():
    spec = importlib.util.spec_from_file_location("bucky_vpn_process_integration", HARNESS)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import {HARNESS}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def install_multipass_timeout_guard(harness) -> None:
    """Keep every Multipass CLI call bounded, including setup diagnostics."""

    multipass_prefix = tuple(harness.multipass_command())
    original_run_host = harness.run_host
    original_checked_host = harness.checked_host

    def bounded_timeout(command: list[str], requested: int) -> int:
        if tuple(command[: len(multipass_prefix)]) != multipass_prefix:
            return requested
        operation_index = len(multipass_prefix)
        operation = command[operation_index] if len(command) > operation_index else ""
        maximum = (
            MAX_MULTIPASS_INFO_TIMEOUT_SEC
            if operation == "info"
            else MAX_MULTIPASS_COMMAND_TIMEOUT_SEC
        )
        return min(requested, maximum)

    def bounded_run_host(command: list[str], timeout_sec: int, capture: bool = False):
        return original_run_host(command, bounded_timeout(command, timeout_sec), capture)

    def bounded_checked_host(
        command: list[str], timeout_sec: int, capture: bool = False
    ):
        return original_checked_host(
            command, bounded_timeout(command, timeout_sec), capture
        )

    harness.run_host = bounded_run_host
    harness.checked_host = bounded_checked_host


def remote_pid(harness, process) -> int:
    result = harness.checked_host(
        [
            *harness.multipass_command(),
            "exec",
            process.instance.name,
            "--",
            "cat",
            process.pid_path,
        ],
        timeout_sec=20,
        capture=True,
    )
    return int(result.stdout.strip())


def assert_process_identity(harness, label: str, process, expected_pid: int) -> None:
    if process.poll() is not None:
        raise harness.IntegrationError(f"{label} stopped during SN restart")
    actual_pid = remote_pid(harness, process)
    if actual_pid != expected_pid:
        raise harness.IntegrationError(
            f"{label} restarted during SN restart: expected pid {expected_pid}, got {actual_pid}"
        )


def matching_remote_log_lines(harness, process, pattern: re.Pattern[str]) -> list[str]:
    result = harness.checked_host(
        [
            *harness.multipass_command(),
            "exec",
            process.instance.name,
            "--",
            "sudo",
            "python3",
            "-c",
            REMOTE_MATCHING_LOG_LINES,
            process.log_path,
            pattern.pattern,
        ],
        timeout_sec=20,
        capture=True,
    )
    return [str(line) for line in json.loads(result.stdout)]


def client_version_commits(harness, process) -> list[dict[str, int]]:
    commits = []
    for line in matching_remote_log_lines(harness, process, CLIENT_COMMIT_PATTERN):
        match = CLIENT_COMMIT_PATTERN.search(line)
        if match is None:
            continue
        commits.append(
            {
                "info_version": int(match.group(1)),
                "pn_info_version": int(match.group(2)),
            }
        )
    return commits


def wait_for_sn_initialized_versions(
    harness,
    process,
    expected_node_ids: set[str],
    restart_started_seconds: int,
    timeout_sec: int,
) -> dict[str, dict[str, int]]:
    deadline = time.monotonic() + timeout_sec
    last_seen: dict[str, dict[str, int]] = {}
    while time.monotonic() < deadline:
        for line in matching_remote_log_lines(harness, process, SN_INITIALIZED_PATTERN):
            match = SN_INITIALIZED_PATTERN.search(line)
            if match is None or match.group(1) not in expected_node_ids:
                continue
            last_seen[match.group(1)] = {
                "pn_info_version": int(match.group(2)),
                "network_count": int(match.group(3)),
            }
        if expected_node_ids.issubset(last_seen):
            observed_at_seconds = int(time.time())
            initialized_versions = {
                last_seen[node_id]["pn_info_version"] for node_id in expected_node_ids
            }
            if len(initialized_versions) != 1:
                raise harness.IntegrationError(
                    "restarted SN did not use one startup PN version for all clients: "
                    f"versions={sorted(initialized_versions)!r}"
                )
            for node_id in expected_node_ids:
                version = last_seen[node_id]["pn_info_version"]
                if not 0 < version <= 0xFFFF_FFFF:
                    raise harness.IntegrationError(
                        f"SN initialized non-u32 PN version for {node_id}: {version}"
                    )
                if version < restart_started_seconds:
                    raise harness.IntegrationError(
                        f"SN initialized stale PN version for {node_id}: "
                        f"version={version}, restart_started_seconds={restart_started_seconds}"
                    )
                if version > observed_at_seconds:
                    raise harness.IntegrationError(
                        f"SN initialized future PN version for {node_id}: "
                        f"version={version}, observed_at_seconds={observed_at_seconds}"
                    )
                if (
                    version - restart_started_seconds
                    > MAX_SN_INITIALIZATION_DELAY_SECONDS
                ):
                    raise harness.IntegrationError(
                        f"SN PN version was not initialized near restart for {node_id}: "
                        f"delay_seconds={version - restart_started_seconds}"
                    )
                last_seen[node_id]["restart_delta_seconds"] = (
                    version - restart_started_seconds
                )
            return last_seen
        time.sleep(1)
    raise harness.IntegrationError(
        "timed out waiting for restarted SN PN-version initialization logs: "
        f"expected={sorted(expected_node_ids)!r}, seen={last_seen!r}"
    )


def wait_for_client_version_commit(
    harness,
    label: str,
    process,
    baseline_count: int,
    expected_info_version: int,
    expected_pn_info_version: int,
    timeout_sec: int,
) -> dict[str, int]:
    deadline = time.monotonic() + timeout_sec
    while time.monotonic() < deadline:
        commits = client_version_commits(harness, process)
        if len(commits) > baseline_count:
            commit = commits[baseline_count]
            if commit["pn_info_version"] != expected_pn_info_version:
                raise harness.IntegrationError(
                    f"{label} committed PN version {commit['pn_info_version']} instead of "
                    f"restarted SN version {expected_pn_info_version}"
                )
            if commit["info_version"] != expected_info_version:
                raise harness.IntegrationError(
                    f"{label} changed network info version across SN restart: "
                    f"before={expected_info_version}, after={commit['info_version']}"
                )
            return commit
        time.sleep(1)
    raise harness.IntegrationError(
        f"timed out waiting for {label} to commit restarted SN PN version "
        f"{expected_pn_info_version}"
    )


def run_live_restart(args: argparse.Namespace) -> Path:
    harness = load_harness()
    install_multipass_timeout_guard(harness)
    harness.ensure_multipass_available()
    if args.use_base_image:
        harness.prepare_base_instance(args.base_image_name, "24.04")
    if not args.no_build:
        harness.run(
            ["cargo", "build", "-p", "bucky-vpn", "-p", "bucky-vpn-server", "--locked"],
            args.timeout_sec,
        )

    client_bin = harness.binary_path("bucky-vpn")
    server_bin = harness.binary_path("bucky-vpn-server")
    for binary in (client_bin, server_bin):
        if not binary.exists():
            raise harness.IntegrationError(f"missing binary {binary}")

    scenario = harness.Scenario(
        name="sn-restart-pn-timestamp",
        servers=(
            harness.ServerSpec("control", sn_enabled=True, pn_enabled=False),
            harness.ServerSpec(
                "proxy", sn_enabled=False, pn_enabled=True, control_server="control"
            ),
        ),
        clients=(
            harness.ClientSpec("client-a", ("mesh-a",)),
            harness.ClientSpec("client-b", ("mesh-a",)),
        ),
    )
    run_id = f"pn016-{os.getpid()}"
    temp_parent = ROOT / "test-results/tmp"
    temp_parent.mkdir(parents=True, exist_ok=True)
    temp_root = Path(tempfile.mkdtemp(prefix="pn-sn-restart-", dir=temp_parent))
    instances = []
    processes = []
    succeeded = False
    evidence: dict[str, object] = {
        "scenario": scenario.name,
        "run_id": run_id,
        "started_at_ms": int(time.time() * 1000),
    }
    try:
        instances, server_instances, client_instances = harness.create_scenario_instances(
            run_id,
            scenario,
            args.keep_instances,
            args.base_image_name if args.use_base_image else None,
            args.parallel_instances,
        )
        harness.install_client_underlay_isolation(client_instances)

        server_infos = {}
        server_process_by_name = {}
        for server_spec in scenario.servers:
            upstream = (
                server_infos.get(server_spec.control_server)
                if server_spec.control_server
                else None
            )
            process, info = harness.start_server(
                temp_root / scenario.name,
                server_bin,
                server_spec,
                upstream,
                server_instances[server_spec.name],
            )
            processes.append(process)
            server_infos[server_spec.name] = info
            server_process_by_name[server_spec.name] = process

        control = server_infos["control"]
        harness.wait_and_approve_proxy_nodes(control, 1, timeout_sec=120)
        harness.http_json_remote(
            control["instance"],
            control["base_url"],
            "POST",
            "/add_network",
            {"name": "mesh-a", "ip_addr": "10.1.0.0", "mask": 24},
            token=control["token"],
        )
        networks = harness.http_json_remote(
            control["instance"],
            control["base_url"],
            "GET",
            "/get_networks",
            token=control["token"],
        )
        network_by_name = {item["name"]: item for item in networks}
        if not network_by_name["mesh-a"].get("pn_server"):
            raise harness.IntegrationError("mesh-a has no PN assignment before restart")

        client_runtimes = []
        for index, client_spec in enumerate(scenario.clients):
            runtime = harness.start_client(
                temp_root,
                client_bin,
                scenario,
                client_spec,
                index,
                client_instances[client_spec.name],
            )
            processes.append(runtime.process)
            client_runtimes.append(runtime)
            harness.join_client_networks(control, network_by_name, scenario, runtime)

        expected_names = {
            f"{scenario.name}-{client.spec.name}-{client.index}"
            for client in client_runtimes
        }
        joined = harness.read_joined_node_ids_by_name(
            control["instance"],
            control["data_dir"],
            temp_root,
            "control-joined",
            expected_names,
        )
        for client in client_runtimes:
            client.node_id = joined[
                f"{scenario.name}-{client.spec.name}-{client.index}"
            ]
        network_ips = harness.assign_joined_clients_to_networks(
            control, network_by_name, client_runtimes
        )

        # The daemon loads a newly joined server at startup. This setup restart happens
        # before the measured SN-only restart boundary.
        for client in client_runtimes:
            client.process.stop()
        ready_clients = []
        for client in client_runtimes:
            restarted = harness.start_client(
                temp_root,
                client_bin,
                scenario,
                client.spec,
                client.index,
                client.process.instance,
                env=client.env,
                data_dir=client.data_dir,
                log_suffix="process-ready",
            )
            restarted.node_id = client.node_id
            processes.append(restarted.process)
            ready_clients.append(restarted)
        client_runtimes = ready_clients

        live_servers = [
            server_process_by_name["control"],
            server_process_by_name["proxy"],
        ]
        harness.wait_network_members_registered(
            control, network_by_name, client_runtimes, live_servers, timeout_sec=120
        )
        harness.wait_client_vpn_runtime_ready(client_runtimes, network_ips, timeout_sec=180)
        harness.install_control_underlay_isolation(
            client_instances, server_instances, scenario
        )
        harness.assert_client_data_plane_via_pn(client_runtimes, network_ips, live_servers)

        stable_pids = {
            "proxy": remote_pid(harness, server_process_by_name["proxy"]),
            **{
                client.spec.name: remote_pid(harness, client.process)
                for client in client_runtimes
            },
        }
        pre_restart_commits = {}
        pre_restart_commit_counts = {}
        for client in client_runtimes:
            label = client.spec.name
            commits = client_version_commits(harness, client.process)
            if not commits:
                raise harness.IntegrationError(
                    f"{label} has no committed VPN version log before SN restart"
                )
            pre_restart_commit_counts[label] = len(commits)
            pre_restart_commits[label] = {
                "node_id": client.node_id,
                **commits[-1],
            }
        old_control_pid = remote_pid(harness, server_process_by_name["control"])
        evidence["before_restart"] = {
            "control_pid": old_control_pid,
            "stable_pids": stable_pids,
            "pn_assignment": network_by_name["mesh-a"]["pn_server"],
            "client_version_commits": pre_restart_commits,
            "data_plane_via_pn": "passed",
        }

        restart_started_ms = int(time.time() * 1000)
        restart_started_seconds = restart_started_ms // 1000
        server_process_by_name["control"].stop()
        control_spec = scenario.servers[0]
        new_control_process, new_control = harness.start_server(
            temp_root / scenario.name,
            server_bin,
            control_spec,
            None,
            server_instances["control"],
        )
        processes.append(new_control_process)
        server_process_by_name["control"] = new_control_process
        new_control_pid = remote_pid(harness, new_control_process)
        if new_control_pid == old_control_pid:
            raise harness.IntegrationError("control PID did not change across restart")

        harness.wait_and_approve_proxy_nodes(new_control, 1, timeout_sec=180)
        live_pn_nodes = harness.http_json_remote_retry(
            new_control["instance"],
            new_control["base_url"],
            "GET",
            "/pn_proxy_nodes",
            token=new_control["token"],
            attempts=5,
        )
        live_pn = next(
            (
                item
                for item in live_pn_nodes
                if item.get("live")
                and item.get("status") == "approved"
                and item.get("pn_server", {}).get("endpoints")
            ),
            None,
        )
        if live_pn is None:
            raise harness.IntegrationError(
                f"restarted SN has no live PN endpoints: {live_pn_nodes!r}"
            )
        post_networks = harness.http_json_remote_retry(
            new_control["instance"],
            new_control["base_url"],
            "GET",
            "/get_networks",
            token=new_control["token"],
            attempts=5,
        )
        post_network_by_name = {item["name"]: item for item in post_networks}
        post_assignment = post_network_by_name["mesh-a"].get("pn_server")
        if (
            not post_assignment
            or post_assignment.get("id") != live_pn["pn_server"].get("id")
        ):
            raise harness.IntegrationError(
                "restarted SN did not bind the persisted PN selection to the live PN: "
                f"assignment={post_assignment!r} live={live_pn!r}"
            )

        live_servers = [new_control_process, server_process_by_name["proxy"]]
        harness.wait_network_members_registered(
            new_control,
            post_network_by_name,
            client_runtimes,
            live_servers,
            timeout_sec=180,
        )
        expected_node_ids = {
            str(client.node_id) for client in client_runtimes if client.node_id is not None
        }
        if len(expected_node_ids) != len(client_runtimes):
            raise harness.IntegrationError("missing client node id for PN-version log chain")
        sn_initialized_versions = wait_for_sn_initialized_versions(
            harness,
            new_control_process,
            expected_node_ids,
            restart_started_seconds,
            timeout_sec=90,
        )
        post_restart_commits = {}
        commit_wait_started_ms = int(time.time() * 1000)
        for client in client_runtimes:
            label = client.spec.name
            node_id = str(client.node_id)
            pre_commit = pre_restart_commits[label]
            sn_version = sn_initialized_versions[node_id]["pn_info_version"]
            if sn_version == pre_commit["pn_info_version"]:
                raise harness.IntegrationError(
                    f"restarted SN reused {label}'s cached PN version {sn_version}"
                )
            commit = wait_for_client_version_commit(
                harness,
                label,
                client.process,
                pre_restart_commit_counts[label],
                pre_commit["info_version"],
                sn_version,
                timeout_sec=90,
            )
            post_restart_commits[label] = {
                "node_id": node_id,
                **commit,
            }
        commit_verified_at_ms = int(time.time() * 1000)
        for label, expected_pid in stable_pids.items():
            process = (
                server_process_by_name["proxy"]
                if label == "proxy"
                else next(
                    client.process for client in client_runtimes if client.spec.name == label
                )
            )
            assert_process_identity(harness, label, process, expected_pid)

        harness.assert_client_data_plane_via_pn(client_runtimes, network_ips, live_servers)
        harness.wait_pn_traffic_reported(
            new_control, post_network_by_name, client_runtimes, timeout_sec=120
        )
        harness.assert_client_logs_clean(
            client_runtimes, include_control_refresh_errors=False
        )
        evidence["after_restart"] = {
            "control_pid": new_control_pid,
            "stable_pids": stable_pids,
            "persisted_pn_assignment": post_assignment,
            "live_pn": live_pn,
            "pn_re_registered_online": True,
            "sn_initialized_versions": sn_initialized_versions,
            "client_version_commits": post_restart_commits,
            "version_chain_verified": True,
            "client_commit_wait_ms": commit_verified_at_ms - commit_wait_started_ms,
            "data_plane_via_pn": "passed",
            "restart_started_at_ms": restart_started_ms,
            "verified_at_ms": int(time.time() * 1000),
        }
        succeeded = True
    finally:
        evidence["succeeded"] = succeeded
        evidence["finished_at_ms"] = int(time.time() * 1000)
        RESULT_ROOT.mkdir(parents=True, exist_ok=True)
        result_path = RESULT_ROOT / f"{run_id}.json"
        result_path.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
        print(f"pn-sn-restart evidence: {result_path}")
        for process in reversed(processes):
            process.stop()
        for instance in reversed(instances):
            instance.stop()
        if succeeded and not args.keep_temp:
            shutil.rmtree(temp_root, ignore_errors=True)
        else:
            print(f"pn-sn-restart workdir retained: {temp_root}")
    return result_path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--no-build", action="store_true")
    parser.add_argument("--keep-instances", action="store_true")
    parser.add_argument("--keep-temp", action="store_true")
    parser.add_argument("--timeout-sec", type=int, default=1800)
    parser.add_argument("--parallel-instances", type=int, default=2)
    parser.add_argument(
        "--base-image-name", default="bucky-vpn-integration-base-24-04"
    )
    parser.add_argument(
        "--use-base-image", action=argparse.BooleanOptionalAction, default=True
    )
    args = parser.parse_args()
    run_live_restart(args)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"pn-sn-restart: {exc}", file=sys.stderr)
        raise SystemExit(1)
