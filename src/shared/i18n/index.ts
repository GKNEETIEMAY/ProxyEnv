import type { AppLanguage } from "../types";

export type Locale = Exclude<AppLanguage, "system">;

const en = {
  appName: "ProxyEnv",
  appTagline: "Proxy environment, without the friction.",
  home: "Proxy",
  settings: "Settings",
  minimizeWindow: "Minimize window",
  maximizeWindow: "Maximize window",
  restoreWindow: "Restore window",
  closeWindow: "Close window",
  refresh: "Refresh status",
  currentProxy: "Current proxy",
  proxyClient: "Proxy client",
  autoDetected: "Detected automatically",
  windowsSystemProxy: "Windows system proxy",
  systemProxyReadOnly: "Read-only status; ProxyEnv does not change this setting.",
  systemProxyOn: "On",
  systemProxyOff: "Off",
  detecting: "Detecting local proxy…",
  noProxy: "No listening proxy found",
  noProxyHint: "Start a proxy client, then refresh. Unknown clients use a shared icon.",
  localProxy: "Local proxy",
  listening: "Listening",
  notListening: "Not listening",
  proxyEnvironment: "Proxy environment",
  enabled: "On",
  disabled: "Off",
  enabling: "Applying…",
  environmentOnHint: "New apps inherit this proxy; restart running apps to apply it.",
  environmentOffHint: "New apps connect directly unless configured otherwise.",
  environmentLayerHint: "User environment used by newly started applications.",
  environmentDisabled: "Disabled",
  environmentPartial: "Partially configured",
  environmentEnabled: "Matches active proxy",
  environmentMismatch: "Needs synchronization",
  partialHint: "Only some selected variables are currently configured.",
  mismatchHint: "The saved environment points somewhere other than the active proxy. Nothing was changed automatically.",
  proxySource: "Proxy source",
  autoDetect: "Automatic detection",
  manualProxy: "Manual proxy",
  manualProxyHint: "Enter a local endpoint",
  host: "Host",
  port: "Port",
  protocol: "Protocol",
  applyManualProxy: "Apply manual proxy",
  applyDetectedProxy: "Apply detected proxy",
  syncToActive: "Sync to active proxy",
  disableProxyEnvironment: "Disable proxy environment",
  restorePrevious: "Restore previous values",
  variables: "Environment variables",
  variablesHint: "Choose which variables the next Apply or Sync action will write.",
  writeVariable: "Write {name}",
  aboutVariable: "About {name}",
  httpProxyDescription: "Used by programs making HTTP requests. This is the safest general default for command-line tools.",
  httpsProxyDescription: "Used for HTTPS destinations. An http:// value is normal: clients create an encrypted tunnel through the local proxy.",
  allProxyDescription: "Fallback for protocols without their own proxy setting. It has the broadest reach and can disrupt in-app or package search, local-network discovery, or apps with limited SOCKS support, so it stays off by default.",
  noProxyDescription: "Lists addresses that should bypass the proxy, such as localhost. ProxyEnv currently displays this value but does not modify it.",
  unset: "Not set",
  operationFailed: "The change could not be applied",
  attention: "Attention needed",
  retryHint: "Check the proxy client, then try again.",
  localOnly: "Everything stays on this device.",
  settingsTitle: "Settings",
  settingsIntro: "Make ProxyEnv feel at home on this computer.",
  back: "Back",
  appearance: "Language & appearance",
  appearanceHint: "Changes apply immediately and save automatically.",
  interfaceLanguage: "Interface language",
  languageHint: "If the system language is unavailable, English is used.",
  system: "Follow system",
  chinese: "简体中文",
  english: "English",
  japanese: "日本語",
  korean: "한국어",
  theme: "Appearance theme",
  themeSystem: "Follow system",
  themeLight: "Light",
  themeDark: "Dark",
  windowBehavior: "Window behavior",
  windowBehaviorHint: "Choose how ProxyEnv starts and leaves the desktop.",
  launchAtStartup: "Launch at startup",
  launchAtStartupHint: "Start ProxyEnv automatically when you sign in.",
  silentStart: "Start silently",
  silentStartHint: "When launched at startup, stay in the system tray.",
  closeToTray: "Close to tray",
  closeToTrayHint: "The close button hides the window instead of quitting.",
  saveFailed: "Settings could not be saved",
  settingsLoadFailed: "Saved settings could not be read",
  copyEndpoint: "Copy proxy address",
  endpointCopied: "Proxy address copied",
  copyFailed: "Proxy address could not be copied",
  secondInstanceOpened: "ProxyEnv is already running. The existing window is now open.",
  general: "General",
  about: "About",
  aboutIntro: "View version information and the current update status.",
  version: "Version",
  updateStatus: "Update status",
  updateSource: "Update source",
  notChecked: "Not checked yet",
  checkForUpdates: "Check for updates",
  checkingUpdates: "Checking for updates…",
  latestVersion: "You are using the latest published version.",
  updateAvailable: "Version {version} is available.",
  noPublishedRelease: "No public release is available yet.",
  updateCheckFailed: "Could not check GitHub Releases.",
  changelog: "Changelog",
  developmentPreview: "Development preview",
  changelogDiscovery: "Detect local proxy clients, listening ports, and protocols automatically.",
  changelogVariables: "Safely toggle and selectively manage HTTP, HTTPS, and ALL proxy variables.",
  changelogDesktop: "Add tray controls, four interface languages, themes, and persistent window behavior.",
  autoConfidence: "confidence"
} as const;

export type Copy = { [K in keyof typeof en]: string };

const zh: Copy = {
  appName: "境启 ProxyEnv", appTagline: "让代理环境切换回归简单。", home: "代理", settings: "设置", minimizeWindow: "最小化窗口", maximizeWindow: "最大化窗口", restoreWindow: "还原窗口", closeWindow: "关闭窗口", refresh: "刷新状态",
  currentProxy: "当前代理", proxyClient: "代理客户端", autoDetected: "自动检测", windowsSystemProxy: "Windows 系统代理", systemProxyReadOnly: "只读状态，ProxyEnv 不会修改此设置。", systemProxyOn: "已开启", systemProxyOff: "已关闭", detecting: "正在检测本机代理…", noProxy: "未发现正在监听的代理", noProxyHint: "启动代理客户端后刷新；未知客户端会使用通用图标。", localProxy: "本机代理", listening: "正在监听", notListening: "未监听",
  proxyEnvironment: "代理环境变量", enabled: "已开启", disabled: "已关闭", enabling: "正在应用…", environmentOnHint: "新启动的应用会继承此代理地址，已运行的应用需要重启才生效。", environmentOffHint: "未单独配置的应用将直接连接网络。", environmentLayerHint: "供新启动应用继承的用户环境变量。", environmentDisabled: "已关闭", environmentPartial: "部分配置", environmentEnabled: "与活动代理一致", environmentMismatch: "需要同步", partialHint: "当前只配置了部分选中的环境变量。", mismatchHint: "环境变量与活动代理地址不一致，ProxyEnv 未自动修改。", proxySource: "代理来源", autoDetect: "自动检测", manualProxy: "手动代理", manualProxyHint: "输入本机代理端点", host: "主机", port: "端口", protocol: "协议", applyManualProxy: "应用手动代理", applyDetectedProxy: "应用检测到的代理", syncToActive: "同步到活动代理", disableProxyEnvironment: "关闭代理环境", restorePrevious: "恢复之前配置",
  variables: "环境变量", variablesHint: "选择下次“应用”或“同步”操作要写入的变量。", writeVariable: "写入 {name}", aboutVariable: "了解 {name}", httpProxyDescription: "供发起 HTTP 请求的程序使用，是命令行工具最稳妥的通用默认项。", httpsProxyDescription: "供访问 HTTPS 地址的程序使用。值以 http:// 开头是正常的：程序会通过本机代理建立加密隧道。", allProxyDescription: "作为没有独立代理设置的协议的回退项，影响范围最广，可能干扰应用内或软件包搜索、局域网发现，以及不完整支持 SOCKS 的程序，因此默认关闭。", noProxyDescription: "列出 localhost 等应绕过代理的地址。ProxyEnv 目前只显示该值，不会修改。", unset: "未设置", operationFailed: "未能完成操作", attention: "需要注意", retryHint: "请检查代理客户端后重试。", localOnly: "所有数据仅在本机处理。",
  settingsTitle: "设置", settingsIntro: "让 ProxyEnv 更贴合这台电脑与您的使用习惯。", back: "返回",
  appearance: "语言与外观", appearanceHint: "修改后立即生效并自动保存。", interfaceLanguage: "界面语言", languageHint: "系统语言无法匹配时使用英文。", system: "跟随系统", chinese: "简体中文", english: "English", japanese: "日本語", korean: "한국어",
  theme: "外观主题", themeSystem: "跟随系统", themeLight: "浅色", themeDark: "深色", windowBehavior: "窗口行为", windowBehaviorHint: "设置 ProxyEnv 启动与离开桌面时的方式。",
  launchAtStartup: "开机自启", launchAtStartupHint: "登录系统后自动运行 ProxyEnv。", silentStart: "静默启动", silentStartHint: "开机自启时不显示主窗口，仅在系统托盘运行。", closeToTray: "关闭时最小化到托盘", closeToTrayHint: "点击关闭按钮时隐藏窗口，而不是退出应用。",
  saveFailed: "设置保存失败", settingsLoadFailed: "无法读取已保存的设置", copyEndpoint: "复制代理地址", endpointCopied: "代理地址已复制", copyFailed: "无法复制代理地址", secondInstanceOpened: "ProxyEnv 已在运行，已为您打开现有窗口。", general: "通用", about: "关于", aboutIntro: "查看版本信息与当前更新状态。", version: "版本", updateStatus: "更新状态", updateSource: "更新来源", notChecked: "尚未检查", checkForUpdates: "检查更新", checkingUpdates: "正在检查更新…", latestVersion: "当前已是最新发布版本。", updateAvailable: "发现新版本 {version}。", noPublishedRelease: "目前还没有公开发布版本。", updateCheckFailed: "无法检查 GitHub Releases。", changelog: "更新日志", developmentPreview: "开发预览", changelogDiscovery: "自动识别本机代理客户端、实际监听端口与代理协议。", changelogVariables: "安全启停并按需管理 HTTP、HTTPS 与 ALL 代理变量。", changelogDesktop: "新增托盘控制、四种界面语言、主题与持久化窗口行为。", autoConfidence: "置信度"
};

const ja: Copy = {
  appName: "ProxyEnv", appTagline: "プロキシ環境を、もっと手軽に。", home: "プロキシ", settings: "設定", minimizeWindow: "ウィンドウを最小化", maximizeWindow: "ウィンドウを最大化", restoreWindow: "ウィンドウを元に戻す", closeWindow: "ウィンドウを閉じる", refresh: "状態を更新",
  currentProxy: "現在のプロキシ", proxyClient: "プロキシクライアント", autoDetected: "自動検出", windowsSystemProxy: "Windows システムプロキシ", systemProxyReadOnly: "読み取り専用。ProxyEnv は変更しません。", systemProxyOn: "オン", systemProxyOff: "オフ", detecting: "ローカルプロキシを検出中…", noProxy: "待機中のプロキシが見つかりません", noProxyHint: "プロキシクライアントを起動して更新してください。不明なクライアントには共通アイコンを使います。", localProxy: "ローカルプロキシ", listening: "待機中", notListening: "停止中",
  proxyEnvironment: "プロキシ環境変数", enabled: "オン", disabled: "オフ", enabling: "適用中…", environmentOnHint: "新規アプリにこのプロキシを継承。起動中のアプリは再起動後に反映。", environmentOffHint: "個別設定のないアプリは直接接続します。", environmentLayerHint: "新しく起動するアプリが継承するユーザー環境です。", environmentDisabled: "無効", environmentPartial: "一部のみ設定", environmentEnabled: "稼働中のプロキシと一致", environmentMismatch: "同期が必要", partialHint: "選択した変数の一部だけが設定されています。", mismatchHint: "環境変数と稼働中のプロキシが一致しません。自動変更はしていません。", proxySource: "プロキシ元", autoDetect: "自動検出", manualProxy: "手動プロキシ", manualProxyHint: "ローカルエンドポイントを入力", host: "ホスト", port: "ポート", protocol: "プロトコル", applyManualProxy: "手動プロキシを適用", applyDetectedProxy: "検出したプロキシを適用", syncToActive: "稼働中のプロキシへ同期", disableProxyEnvironment: "プロキシ環境を無効化", restorePrevious: "以前の設定を復元",
  variables: "環境変数", variablesHint: "次回の適用または同期で書き込む変数を選択します。", writeVariable: "{name} を書き込む", aboutVariable: "{name} について", httpProxyDescription: "HTTP リクエストを行うプログラムで使われます。コマンドラインツール向けの安全な標準設定です。", httpsProxyDescription: "HTTPS 接続先で使われます。http:// の値でも正常です。クライアントはローカルプロキシ経由で暗号化トンネルを作ります。", allProxyDescription: "個別のプロキシ設定がないプロトコルのフォールバックです。影響範囲が最も広く、アプリ内・パッケージ検索、LAN 検出、SOCKS 対応が不完全なアプリを妨げる場合があるため、既定ではオフです。", noProxyDescription: "localhost などプロキシを迂回するアドレスです。現在 ProxyEnv は表示のみ行い、変更しません。", unset: "未設定", operationFailed: "変更を適用できませんでした", attention: "確認が必要です", retryHint: "プロキシクライアントを確認して再試行してください。", localOnly: "データはすべてこの端末内で処理されます。",
  settingsTitle: "設定", settingsIntro: "このコンピューターに合うよう ProxyEnv を整えます。", back: "戻る",
  appearance: "言語と外観", appearanceHint: "変更はすぐに反映され、自動的に保存されます。", interfaceLanguage: "表示言語", languageHint: "システム言語に対応していない場合は英語を使用します。", system: "システムに従う", chinese: "简体中文", english: "English", japanese: "日本語", korean: "한국어",
  theme: "外観テーマ", themeSystem: "システムに従う", themeLight: "ライト", themeDark: "ダーク", windowBehavior: "ウィンドウ動作", windowBehaviorHint: "ProxyEnv の起動方法と閉じ方を選びます。",
  launchAtStartup: "起動時に実行", launchAtStartupHint: "サインイン時に ProxyEnv を自動起動します。", silentStart: "サイレント起動", silentStartHint: "自動起動時はウィンドウを表示せずトレイに常駐します。", closeToTray: "閉じるとトレイへ", closeToTrayHint: "閉じるボタンで終了せずウィンドウを隠します。",
  saveFailed: "設定を保存できませんでした", settingsLoadFailed: "保存済みの設定を読み込めませんでした", copyEndpoint: "プロキシアドレスをコピー", endpointCopied: "プロキシアドレスをコピーしました", copyFailed: "プロキシアドレスをコピーできませんでした", secondInstanceOpened: "ProxyEnv はすでに実行中です。既存のウィンドウを開きました。", general: "一般", about: "情報", aboutIntro: "バージョン情報と現在の更新状況を確認します。", version: "バージョン", updateStatus: "更新状況", updateSource: "更新元", notChecked: "未確認", checkForUpdates: "更新を確認", checkingUpdates: "更新を確認中…", latestVersion: "最新の公開バージョンです。", updateAvailable: "バージョン {version} を利用できます。", noPublishedRelease: "公開リリースはまだありません。", updateCheckFailed: "GitHub Releases を確認できませんでした。", changelog: "変更履歴", developmentPreview: "開発プレビュー", changelogDiscovery: "ローカルプロキシクライアント、待受ポート、プロトコルを自動検出します。", changelogVariables: "HTTP、HTTPS、ALL プロキシ変数を安全に切り替え、選択して管理します。", changelogDesktop: "トレイ操作、4 言語、テーマ、永続的なウィンドウ動作を追加しました。", autoConfidence: "信頼度"
};

const ko: Copy = {
  appName: "ProxyEnv", appTagline: "프록시 환경 전환을 더 간단하게.", home: "프록시", settings: "설정", minimizeWindow: "창 최소화", maximizeWindow: "창 최대화", restoreWindow: "창 복원", closeWindow: "창 닫기", refresh: "상태 새로고침",
  currentProxy: "현재 프록시", proxyClient: "프록시 클라이언트", autoDetected: "자동 감지", windowsSystemProxy: "Windows 시스템 프록시", systemProxyReadOnly: "읽기 전용 상태이며 ProxyEnv는 변경하지 않습니다.", systemProxyOn: "켜짐", systemProxyOff: "꺼짐", detecting: "로컬 프록시 감지 중…", noProxy: "수신 중인 프록시를 찾지 못했습니다", noProxyHint: "프록시 클라이언트를 시작한 후 새로고침하세요. 알 수 없는 클라이언트는 공용 아이콘을 사용합니다.", localProxy: "로컬 프록시", listening: "수신 중", notListening: "수신 안 함",
  proxyEnvironment: "프록시 환경 변수", enabled: "켜짐", disabled: "꺼짐", enabling: "적용 중…", environmentOnHint: "새 앱은 이 프록시를 상속합니다. 실행 중인 앱은 재시작 후 적용됩니다.", environmentOffHint: "별도 설정이 없는 앱은 직접 연결합니다.", environmentLayerHint: "새로 시작하는 앱이 상속하는 사용자 환경입니다.", environmentDisabled: "비활성", environmentPartial: "일부 설정됨", environmentEnabled: "활성 프록시와 일치", environmentMismatch: "동기화 필요", partialHint: "선택한 변수 중 일부만 설정되어 있습니다.", mismatchHint: "환경 변수와 활성 프록시 주소가 다릅니다. 자동 변경하지 않았습니다.", proxySource: "프록시 원본", autoDetect: "자동 감지", manualProxy: "수동 프록시", manualProxyHint: "로컬 엔드포인트 입력", host: "호스트", port: "포트", protocol: "프로토콜", applyManualProxy: "수동 프록시 적용", applyDetectedProxy: "감지된 프록시 적용", syncToActive: "활성 프록시로 동기화", disableProxyEnvironment: "프록시 환경 끄기", restorePrevious: "이전 설정 복원",
  variables: "환경 변수", variablesHint: "다음 적용 또는 동기화 작업에서 기록할 변수를 선택하세요.", writeVariable: "{name} 기록", aboutVariable: "{name} 정보", httpProxyDescription: "HTTP 요청을 보내는 프로그램이 사용합니다. 명령줄 도구에 가장 안전한 기본 옵션입니다.", httpsProxyDescription: "HTTPS 대상에 사용합니다. http:// 값도 정상입니다. 클라이언트가 로컬 프록시를 통해 암호화 터널을 만듭니다.", allProxyDescription: "별도 프록시 설정이 없는 프로토콜의 대체 설정입니다. 영향 범위가 가장 넓어 앱 내·패키지 검색, 로컬 네트워크 검색, SOCKS 지원이 제한된 앱을 방해할 수 있으므로 기본값은 꺼짐입니다.", noProxyDescription: "localhost처럼 프록시를 우회할 주소 목록입니다. 현재 ProxyEnv는 이 값을 표시만 하고 수정하지 않습니다.", unset: "설정 안 됨", operationFailed: "변경 사항을 적용하지 못했습니다", attention: "확인이 필요합니다", retryHint: "프록시 클라이언트를 확인한 후 다시 시도하세요.", localOnly: "모든 데이터는 이 기기에서만 처리됩니다.",
  settingsTitle: "설정", settingsIntro: "이 컴퓨터와 사용 방식에 맞게 ProxyEnv를 조정하세요.", back: "뒤로",
  appearance: "언어 및 화면", appearanceHint: "변경 사항은 즉시 적용되고 자동으로 저장됩니다.", interfaceLanguage: "인터페이스 언어", languageHint: "시스템 언어를 지원하지 않으면 영어를 사용합니다.", system: "시스템 설정 사용", chinese: "简体中文", english: "English", japanese: "日本語", korean: "한국어",
  theme: "화면 테마", themeSystem: "시스템 설정 사용", themeLight: "라이트", themeDark: "다크", windowBehavior: "창 동작", windowBehaviorHint: "ProxyEnv가 시작되고 닫히는 방식을 선택하세요.",
  launchAtStartup: "시작 시 실행", launchAtStartupHint: "로그인할 때 ProxyEnv를 자동 실행합니다.", silentStart: "조용히 시작", silentStartHint: "자동 시작 시 창을 표시하지 않고 시스템 트레이에서 실행합니다.", closeToTray: "닫을 때 트레이로", closeToTrayHint: "닫기 버튼을 누르면 종료하지 않고 창을 숨깁니다.",
  saveFailed: "설정을 저장하지 못했습니다", settingsLoadFailed: "저장된 설정을 읽지 못했습니다", copyEndpoint: "프록시 주소 복사", endpointCopied: "프록시 주소를 복사했습니다", copyFailed: "프록시 주소를 복사하지 못했습니다", secondInstanceOpened: "ProxyEnv가 이미 실행 중입니다. 기존 창을 열었습니다.", general: "일반", about: "정보", aboutIntro: "버전 정보와 현재 업데이트 상태를 확인합니다.", version: "버전", updateStatus: "업데이트 상태", updateSource: "업데이트 소스", notChecked: "확인하지 않음", checkForUpdates: "업데이트 확인", checkingUpdates: "업데이트 확인 중…", latestVersion: "최신 공개 버전을 사용 중입니다.", updateAvailable: "버전 {version}을 사용할 수 있습니다.", noPublishedRelease: "아직 공개 릴리스가 없습니다.", updateCheckFailed: "GitHub Releases를 확인하지 못했습니다.", changelog: "변경 기록", developmentPreview: "개발 미리보기", changelogDiscovery: "로컬 프록시 클라이언트, 수신 포트와 프로토콜을 자동으로 감지합니다.", changelogVariables: "HTTP, HTTPS 및 ALL 프록시 변수를 안전하게 전환하고 선택 관리합니다.", changelogDesktop: "트레이 제어, 4개 언어, 테마와 지속되는 창 동작을 추가했습니다.", autoConfidence: "신뢰도"
};

export const messages: Record<Locale, Copy> = { en, "zh-CN": zh, ja, ko };
export type MessageKey = keyof Copy;

export function resolveLocale(language: AppLanguage): Locale {
  if (language !== "system") return language;
  const locale = navigator.language.toLowerCase();
  if (locale === "zh-cn" || locale === "zh-sg" || locale.startsWith("zh-hans")) return "zh-CN";
  if (locale.startsWith("ja")) return "ja";
  if (locale.startsWith("ko")) return "ko";
  return "en";
}
