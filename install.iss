#define AppName "BuckyVPN"
#define AppVersion "1.0.0"

[Setup]
AppName={#AppName}
AppVersion={#AppVersion}
WizardStyle=modern
DefaultDirName={autopf}\bucky-vpn
DefaultGroupName=bucky-vpn
UninstallDisplayIcon={app}\bucky-vpn.exe
Compression=lzma2
SolidCompression=yes
OutputDir=./dist
OutputBaseFilename={#AppName}_{#AppVersion}_Setup
VersionInfoVersion={#AppVersion}
SetupIconFile=vpn-client/bucky-vpn.ico

[Files]
Source: "target/release/bucky-vpn.exe"; DestDir: "{app}"; Flags: ignoreversion restartreplace
Source: "wintun.dll"; DestDir: "{app}"

[Setup]
CloseApplications=yes
CloseApplicationsFilter=bucky-vpn.exe

[Icons]
Name: "{group}\bucky-vpn.exe"; Filename: "{app}\bucky-vpn.exe"

[Run]
; 使用 sc.exe 创建服务
Filename: "{sys}\sc.exe"; Parameters: "create BuckyVPN binPath= ""\""{app}\bucky-vpn.exe\"" daemon"" start= auto"; Flags: runhidden waituntilterminated
; 启动服务
Filename: "{sys}\sc.exe"; Parameters: "start BuckyVPN"; Flags: runhidden waituntilterminated

[UninstallRun]
; 停止服务
Filename: "{sys}\sc.exe"; Parameters: "stop BuckyVPN"; RunOnceId: "CleanupOnce"; Flags: runhidden waituntilterminated
; 删除服务
Filename: "{sys}\sc.exe"; Parameters: "delete BuckyVPN"; RunOnceId: "CleanupOnce"; Flags: runhidden waituntilterminated
Filename: "taskkill"; Parameters: "/f /im bucky-vpn.exe"; RunOnceId: "CleanupOnce"; Flags: runhidden waituntilterminated

[Code]
procedure AddToPath(Path: string);
var
  PathVar: string;
begin
  // 获取当前的 PATH 环境变量
  if not RegQueryStringValue(HKEY_LOCAL_MACHINE, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', PathVar) then
  begin
    PathVar := '';
  end;

  // 检查路径是否已经存在于 PATH 中
  if Pos(Path, PathVar) = 0 then
  begin
    // 如果不存在，则添加到 PATH 中
    PathVar := Path + ';' + PathVar;
    // 设置新的 PATH 环境变量
    RegWriteStringValue(HKEY_LOCAL_MACHINE, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', PathVar);
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    // 安装完成后，将安装目录添加到 PATH 环境变量
    AddToPath(ExpandConstant('{app}'));
  end;
end;

procedure RemoveFromPath(Path: string);
var
  PathVar: string;
  NewPath: string;
  PosPath: Integer;
begin
  // 获取当前的 PATH 环境变量
  if not RegQueryStringValue(HKEY_LOCAL_MACHINE, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', PathVar) then
  begin
    Exit; // 如果 PATH 不存在，直接退出
  end;

  // 查找路径在 PATH 中的位置
  PosPath := Pos(Path, PathVar);
  if PosPath > 0 then
  begin
    // 移除路径
    NewPath := PathVar;
    Delete(NewPath, PosPath, Length(Path));

    // 移除多余的分号
    if (PosPath > 1) and (NewPath[PosPath - 1] = ';') then
    begin
      Delete(NewPath, PosPath - 1, 1);
    end
    else if (PosPath < Length(NewPath)) and (NewPath[PosPath] = ';') then
    begin
      Delete(NewPath, PosPath, 1);
    end;

    // 更新 PATH 环境变量
    RegWriteStringValue(HKEY_LOCAL_MACHINE, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', NewPath);
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    // 卸载完成后，从 PATH 环境变量中移除安装目录
    RemoveFromPath(ExpandConstant('{app}'));
    Sleep(1000); // 等待 1 秒
    DeleteFile(ExpandConstant('{app}\bucky-vpn.exe'));
    DelTree(ExpandConstant('{app}\data\logs'), True, True, True);
  end;
end;
