; Inno Setup 自动化静默安装及环境变量配置脚本
#define MyAppName "Md2Proj"
#define MyAppVersion "1.0.0"
#define MyAppExeName "md2proj.exe"

[Setup]
AppName={#MyAppName}
AppVersion={#MyAppVersion}
; 自动安装到当前用户的局部应用数据目录，避免触发管理员 UAC 弹窗
DefaultDirName={localappdata}\{#MyAppName}
DefaultGroupName={#MyAppName}
OutputBaseFilename=Md2Proj-Installer
Compression=lzma
SolidCompression=yes
ArchitecturesAllowed=x64
ArchitecturesInstallIn64BitMode=x64

; 【自动安装支持】关闭所有的安装向导交互页面，使得双击后立刻自动完成安装
DisableWelcomePage=yes
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=yes
DisableFinishedPage=yes

PrivilegesRequired=lowest
; 安装完成立刻向操作系统广播，允许 Shell (CMD/PowerShell) 载入新的 PATH 变量
ChangesEnvironment=yes

[Files]
Source: "target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Registry]
; 自动向当前 Windows 用户的环境变量 PATH 中追加安装路径
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Check: NeedsAddPath(ExpandConstant('{app}'))

[Code]
// 检测逻辑：检查 Path 环境变量中是否已经存在程序目录，避免重复写入
function NeedsAddPath(Param: string): boolean;
var
  OrigPath: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath) then
  begin
    Result := True;
    exit;
  end;
  // 前后加分号以确保精确匹配，防止包含类似字眼的其它目录被误判
  Result := Pos(';' + UpperCase(Param) + ';', ';' + UpperCase(OrigPath) + ';') = 0;
end;
