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
Source: "target/release/bucky-vpn.exe"; DestDir: "{app}"
Source: "wintun.dll"; DestDir: "{app}"

[Icons]
Name: "{group}\bucky-vpn.exe"; Filename: "{app}\bucky-vpn.exe"

[Run]
; 使用 sc.exe 创建服务
Filename: "{sys}\sc.exe"; Parameters: "create BuckyVPN binPath= ""{app}\bucky-vpn.exe"" start= auto"; Flags: runhidden waituntilterminated
; 启动服务
Filename: "{sys}\sc.exe"; Parameters: "start BuckyVPN"; Flags: runhidden waituntilterminated

[UninstallRun]
; 停止服务
Filename: "{sys}\sc.exe"; Parameters: "stop BuckyVPN"; Flags: runhidden waituntilterminated
; 删除服务
Filename: "{sys}\sc.exe"; Parameters: "delete BuckyVPN"; Flags: runhidden waituntilterminated

[Code]
function GetEnvValue(EnvVarName: String): String;
begin
  Result := GetEnv(EnvVarName);
  MsgBox('Environment variable ' + EnvVarName + ' not found!', mbError, MB_OK);
end;
