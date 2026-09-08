const extensionEn = {
  rbExtConfirmRestore: 'Confirm restore',
  rbExtRestoreTitle: 'Choose configuration to restore', rbExtRestoreScope: 'Restore only the selected files from their remote backups. Changes to the file or backup stop restoration.',
  rbExtRestoreImpact: 'Reverts ProxyEnv configuration changes. After reloading, new sessions use the restored configuration and routing; requests may no longer pass through the bridge.',
  rbExtRestored: 'Restored. Reload VS Code and start a new session to use the original configuration. This does not disconnect the tunnel or verify model routing.',
  rbExtTitle: 'Choose configuration scope', rbExtCli: 'CLI', rbExtGui: 'VS Code Extension',
  rbExtScope: 'Choose only the surfaces you want to change. Each file has its own preview and transaction; partial success is reported separately.',
  rbExtInspect: 'Inspect remote extensions', rbExtDetected: 'Installed', rbExtUnknown: 'Not checked', rbExtUnsupported: 'Missing, ambiguous or unsupported runtime',
  rbExtLocation: 'In the target Remote - SSH window, open Developer: Show Running Extensions. Confirm this extension runs remotely under the account shown below. Also confirm Open config.toml (Codex) or Open Remote Settings (Claude) uses the displayed path. Custom server/config directories are not supported.',
  rbExtConfirmLocation: 'I confirmed the remote execution account and configuration path in VS Code',
  rbExtCodexImpact: 'Changes model_provider in the shared remote Codex config. Other Codex sessions using defaults will also use the bridge. Model, permissions and MCP settings are preserved.',
  rbExtClaudeImpact: 'Adds the gateway endpoint and public PROXY_MANAGED placeholder to remote claudeCode.environmentVariables. Existing routing or credential entries cause a conflict. No real key is copied.',
  rbExtPending: 'Written · reload and model verification required', rbExtNotConfigured: 'Not configured', rbExtReview: 'Preview selected files',
  rbExtRestart: 'Reload the VS Code window and start a new extension session. File verification does not verify model routing. Disconnect keeps the bridge endpoint; restore is a separate action.',
  rbExtPartial: 'Some files succeeded. Review each result below; failed files require a new preview.',
  rbExtOpaque: 'Existing configuration is retained in the remote backup. Unrelated content and credentials are not returned.',
  rbExtRestoreOpaque: 'Restore the original remote file after hash verification; original values are not returned.',
  rbExtRestoreAbsent: 'Remove the file created by ProxyEnv.', rbExtError: 'Could not safely configure this extension. Recheck the remote execution location, default paths, installed versions and existing routing settings. No software was installed.',
  rbExtApplied: 'Applied', rbExtFailed: 'Failed', rbExtWaiting: 'Not applied', rbExtOpened: 'VS Code open requested',
};
type ExtensionLabels = { [K in keyof typeof extensionEn]: string };
const extensionZh: ExtensionLabels = {
  rbExtConfirmRestore:'确认恢复',
  rbExtRestoreTitle:'选择要恢复的配置',rbExtRestoreScope:'仅从远端备份恢复所选文件。文件或备份已被修改时停止恢复。',
  rbExtRestoreImpact:'撤销 ProxyEnv 的配置改动。重载后新会话使用恢复的原配置及路由，后续请求可能不再经过桥接。',
  rbExtRestored:'已恢复。请重载 VS Code 并新建会话以使用原配置。此次操作不会断开隧道，也不验证模型路由。',
  rbExtTitle:'选择配置范围',rbExtCli:'CLI',rbExtGui:'VS Code 图形化扩展',
  rbExtScope:'只选择需要修改的入口。各文件分别预览和执行事务，部分成功会逐项显示。',
  rbExtInspect:'检查远端扩展',rbExtDetected:'已检测安装',rbExtUnknown:'未检查',rbExtUnsupported:'未安装、版本不唯一或运行时不受支持',
  rbExtLocation:'在目标 Remote - SSH 窗口运行 Developer: Show Running Extensions，确认该扩展运行在下方账户的远端。再确认 Codex 的 Open config.toml 或 Claude 的 Open Remote Settings 使用所示路径。暂不支持自定义 Server 或配置目录。',
  rbExtConfirmLocation:'我已在 VS Code 确认远端执行账户与配置路径',
  rbExtCodexImpact:'将修改远端共享 Codex 配置的 model_provider。同账户其他使用默认配置的 Codex 会话也会走桥接；保留模型、权限和 MCP 配置。',
  rbExtClaudeImpact:'向远端 claudeCode.environmentVariables 增加网关地址及公开占位值 PROXY_MANAGED。已有路由或凭据项会触发冲突，不复制真实密钥。',
  rbExtPending:'已写入 · 待重载及模型验收',rbExtNotConfigured:'未配置',rbExtReview:'预览所选文件',
  rbExtRestart:'请重载 VS Code 窗口并新建扩展会话。文件校验不等于模型路由已验证。断开时保留桥接端点，恢复需单独操作。',
  rbExtPartial:'部分文件已成功。请查看逐项结果；失败项需要重新预览。',
  rbExtOpaque:'原配置保留在远端备份中，不回传无关内容和凭据。',
  rbExtRestoreOpaque:'校验哈希后恢复远端原文件，不回传原值。',rbExtRestoreAbsent:'删除由 ProxyEnv 创建的文件。',
  rbExtError:'无法安全配置此扩展。请重新确认远端运行位置、默认目录、安装版本及已有路由配置。未安装任何软件。',
  rbExtApplied:'已应用',rbExtFailed:'失败',rbExtWaiting:'未应用',rbExtOpened:'已请求打开 VS Code',
};
const extensionJa: ExtensionLabels = {
  rbExtConfirmRestore:'復元を確認',
  rbExtRestoreTitle:'復元する設定を選択',rbExtRestoreScope:'選択したファイルだけをリモートのバックアップから復元します。ファイルやバックアップに変更があれば停止します。',
  rbExtRestoreImpact:'ProxyEnv の設定変更を取り消します。再読込後の新規セッションは元の設定と経路を使い、ブリッジを経由しない場合があります。',
  rbExtRestored:'復元しました。VS Code を再読込して新規セッションで元の設定を使用してください。トンネルの切断やモデル経路の検証は行いません。',
  rbExtTitle:'設定する対象を選択',rbExtCli:'CLI',rbExtGui:'VS Code 拡張機能',
  rbExtScope:'変更する対象だけを選択してください。ファイルごとに確認して適用し、一部の成功も個別に表示します。',
  rbExtInspect:'リモート拡張を確認',rbExtDetected:'インストール確認済み',rbExtUnknown:'未確認',rbExtUnsupported:'未導入・複数バージョン・未対応ランタイム',
  rbExtLocation:'対象の Remote - SSH ウィンドウで Developer: Show Running Extensions を開き、下記アカウントのリモートで動作することを確認してください。Codex の Open config.toml または Claude の Open Remote Settings で表示パスも確認します。独自の Server・設定ディレクトリは未対応です。',
  rbExtConfirmLocation:'VS Code でリモート実行アカウントと設定パスを確認しました',
  rbExtCodexImpact:'共有 Codex 設定の model_provider を変更します。同じアカウントで既定値を使う他のセッションにも影響します。モデル・権限・MCP 設定は保持します。',
  rbExtClaudeImpact:'リモートの claudeCode.environmentVariables に接続先と公開値 PROXY_MANAGED を追加します。既存の経路・認証項目があれば停止します。実際のキーはコピーしません。',
  rbExtPending:'書き込み済み・再読込とモデル検証が必要',rbExtNotConfigured:'未設定',rbExtReview:'選択したファイルを確認',
  rbExtRestart:'VS Code を再読込し、新しい拡張セッションを開始してください。ファイル検証はモデル経路の検証ではありません。切断時も接続先は保持され、復元は別操作です。',
  rbExtPartial:'一部のファイルが成功しました。個別の結果を確認し、失敗した項目を再プレビューしてください。',
  rbExtOpaque:'元の設定はリモートにバックアップされます。無関係の内容と認証情報は返しません。',
  rbExtRestoreOpaque:'ハッシュ確認後に元のファイルを復元します。元の値は返しません。',rbExtRestoreAbsent:'ProxyEnv が作成したファイルを削除します。',
  rbExtError:'安全に設定できませんでした。リモート実行場所、既定パス、バージョン、既存の経路設定を確認してください。ソフトウェアは導入していません。',
  rbExtApplied:'適用済み',rbExtFailed:'失敗',rbExtWaiting:'未適用',rbExtOpened:'VS Code を開く要求を送信しました',
};
const extensionKo: ExtensionLabels = {
  rbExtConfirmRestore:'복원 확인',
  rbExtRestoreTitle:'복원할 설정 선택',rbExtRestoreScope:'선택한 파일만 원격 백업에서 복원합니다. 파일이나 백업이 변경되었으면 중단합니다.',
  rbExtRestoreImpact:'ProxyEnv의 설정 변경을 취소합니다. 다시 로드한 새 세션은 원래 설정과 라우팅을 사용하며 이후 요청은 브리지를 거치지 않을 수 있습니다.',
  rbExtRestored:'복원했습니다. VS Code를 다시 로드하고 새 세션에서 원래 설정을 사용하세요. 터널을 끊거나 모델 라우팅을 검증하지 않습니다.',
  rbExtTitle:'설정 범위 선택',rbExtCli:'CLI',rbExtGui:'VS Code 그래픽 확장',
  rbExtScope:'변경할 대상만 선택하세요. 파일별로 미리 보고 적용하며 일부 성공도 각각 표시합니다.',
  rbExtInspect:'원격 확장 확인',rbExtDetected:'설치 확인됨',rbExtUnknown:'확인 안 됨',rbExtUnsupported:'미설치, 여러 버전 또는 미지원 런타임',
  rbExtLocation:'대상 Remote - SSH 창에서 Developer: Show Running Extensions를 열어 아래 계정의 원격에서 실행되는지 확인하세요. Codex의 Open config.toml 또는 Claude의 Open Remote Settings에서 표시된 경로도 확인하세요. 사용자 지정 Server 및 설정 디렉터리는 지원하지 않습니다.',
  rbExtConfirmLocation:'VS Code에서 원격 실행 계정과 설정 경로를 확인했습니다',
  rbExtCodexImpact:'공유 Codex 설정의 model_provider를 변경합니다. 같은 계정에서 기본 설정을 쓰는 다른 세션에도 영향을 줍니다. 모델, 권한, MCP 설정은 보존합니다.',
  rbExtClaudeImpact:'원격 claudeCode.environmentVariables에 게이트웨이 주소와 공개 값 PROXY_MANAGED를 추가합니다. 기존 라우팅이나 인증 항목이 있으면 중단합니다. 실제 키는 복사하지 않습니다.',
  rbExtPending:'기록됨 · 다시 로드 및 모델 검증 필요',rbExtNotConfigured:'미설정',rbExtReview:'선택한 파일 미리 보기',
  rbExtRestart:'VS Code 창을 다시 로드하고 새 확장 세션을 시작하세요. 파일 검증은 모델 라우팅 검증이 아닙니다. 연결 해제 시 엔드포인트는 유지되며 복원은 별도 작업입니다.',
  rbExtPartial:'일부 파일이 성공했습니다. 각 결과를 확인하고 실패 항목을 다시 미리 보세요.',
  rbExtOpaque:'원래 설정은 원격 백업에 보존됩니다. 무관한 내용과 인증 정보는 반환하지 않습니다.',
  rbExtRestoreOpaque:'해시 확인 후 원래 파일을 복원합니다. 원래 값은 반환하지 않습니다.',rbExtRestoreAbsent:'ProxyEnv가 만든 파일을 삭제합니다.',
  rbExtError:'확장을 안전하게 설정할 수 없습니다. 원격 실행 위치, 기본 경로, 설치 버전 및 기존 라우팅을 확인하세요. 소프트웨어를 설치하지 않았습니다.',
  rbExtApplied:'적용됨',rbExtFailed:'실패',rbExtWaiting:'미적용',rbExtOpened:'VS Code 열기 요청됨',
};
const flowEn = {
  navLocal: "Local environment", navRemote: "Remote bridge", rbPageIntro: "Choose an SSH environment, bridge only the capabilities you need, then follow the verified next steps.",
  rbTargetSource: "Source", rbTargetConfig: "Configuration", rbSourceOpenSsh: "Local OpenSSH", rbSourceVscode: "VS Code Remote", rbSourceMoba: "MobaXterm", rbTargetUnsupported: "Detected · unavailable for bridging", rbMobaUnsupported: "This MobaXterm session cannot be safely converted to OpenSSH. ProxyEnv will not guess its authentication or jump-host settings.",
  rbRegeneratePorts: "Generate new ports", rbPortsGenerated: "Available remote loopback ports generated", rbRemotePortAuto: "Generated after the SSH check and verified again before connecting.", rbPortRace: "A generated remote port was occupied before connection. New ports were generated; review them and connect again.",
  rbCcConfirmed: "CC Switch routing detected", rbCcUnknown: "The port is listening, but the service identity could not be confirmed", rbCcMissing: "CC Switch routing was not detected", rbCcOpenHint: "Open CC Switch → Settings → Advanced → Routing Service, enable Codex / Claude Code routing, then detect again.",
  rbConnectedTitle: "Bridge established", rbNextSteps: "What to do next", rbProxyUseTitle: "General network proxy", rbCcUseTitle: "CC Switch AI routing", rbConfigureBeforeLaunch: "Configure and verify this CLI before using its launch command.", rbStepTerminal: "Open a terminal on the remote server.", rbStepCopy: "Copy the proxy environment variables below.", rbStepPaste: "Paste and run them in the current shell.", rbStepTest: "Test that the remote host can reach the network through your local proxy.", rbStepRun: "Run the CLI that needs network access in that shell.", rbShellScope: "These variables affect only the current shell and its child processes. ProxyEnv does not modify .bashrc, .profile, or other startup files.", rbClaudeOnboarding: "Claude Code first-run onboarding will be marked complete in ~/.claude.json. Project trust is not granted in advance; Claude Code will still ask you to trust the folder before the conversation starts.",
  rbSshHealth: "SSH connection", rbLocalProxyHealth: "Local proxy", rbRemoteProxyHealth: "Remote proxy entry", rbLocalCcHealth: "CC Switch", rbRemoteCcHealth: "Remote CC Switch entry", rbHealthy: "Available", rbPending: "Pending verification", rbNotConfigured: "Not configured", rbDisconnectDetails: "Disconnecting immediately closes the SSH reverse forwards, so the remote loopback ports stop working. Codex and Claude configuration files remain until you restore them separately.",
  rbCcError: "CC Switch routing was not detected on this port. Enable Routing Service in CC Switch and check the local routing port, then try again.", rbBridgeUnavailableError: "The bridge is not currently available. Recheck the SSH target and establish the bridge before using this action.", rbNoCapabilityError: "Select at least one bridge capability before continuing.", rbAlreadyConnectedError: "A bridge is already connected. Disconnect it before creating another one.", rbStateError: "ProxyEnv could not read the bridge state. No settings were changed; close and reopen the page, then retry.", rbProcessError: "The SSH process could not be started or monitored. Check that Windows OpenSSH is installed, then retry.", rbRemoteError: "The remote helper did not return a valid result. Check the remote Linux account and required tools, then retry.", rbNetworkError: "The remote proxy test could not reach the test target. The tunnel remains unchanged; check the local proxy and remote network policy.", rbTargetError: "This remote target is detected but cannot be bridged safely with its current configuration.", rbPortAllocationError: "ProxyEnv could not find two available remote loopback ports. Refresh the target and try again.",
};
type FlowLabels = { [K in keyof typeof flowEn]: string };
const flowZh: FlowLabels = {
  navLocal:"本机环境",navRemote:"远程桥接",rbPageIntro:"选择 SSH 远程环境，只桥接所需能力，并按验证后的步骤继续使用。",
  rbTargetSource:"来源",rbTargetConfig:"配置来源",rbSourceOpenSsh:"本机 OpenSSH",rbSourceVscode:"VS Code Remote",rbSourceMoba:"MobaXterm",rbTargetUnsupported:"已识别 · 当前无法桥接",rbMobaUnsupported:"该 MobaXterm 会话无法安全转换为 OpenSSH。ProxyEnv 不会猜测认证方式或跳板配置。",
  rbRegeneratePorts:"重新生成端口",rbPortsGenerated:"已生成可用的远端 Loopback 端口",rbRemotePortAuto:"SSH 检查后自动生成，并在建立连接前再次确认。",rbPortRace:"远端端口在建立连接前被占用。已重新生成端口，请确认后再次连接。",
  rbCcConfirmed:"已检测到 CC Switch 路由",rbCcUnknown:"端口正在监听，但无法确认服务身份",rbCcMissing:"未检测到 CC Switch 本地路由服务",rbCcOpenHint:"请打开 CC Switch → 设置 → 高级 → Routing Service，开启 Codex / Claude Code Routing 后重新检测。",
  rbConnectedTitle:"桥接已建立",rbNextSteps:"接下来怎么使用",rbProxyUseTitle:"普通网络代理",rbCcUseTitle:"CC Switch AI 路由",rbConfigureBeforeLaunch:"请先配置并验证该 CLI，完成后才可使用启动命令。",rbStepTerminal:"在远程服务器打开终端。",rbStepCopy:"复制下方代理环境变量。",rbStepPaste:"粘贴到当前 Shell 并执行。",rbStepTest:"测试远端能否通过本机代理访问网络。",rbStepRun:"在当前 Shell 运行需要联网的 CLI。",rbShellScope:"环境变量只对当前 Shell 及其子进程生效。ProxyEnv 不修改 .bashrc、.profile 等启动文件。",rbClaudeOnboarding:"将把 ~/.claude.json 中的 Claude Code 首次引导状态标记为已完成，但不会预先信任任何项目目录；启动后仍会正常询问是否信任当前文件夹，再进入对话。",
  rbSshHealth:"SSH 连接",rbLocalProxyHealth:"本机代理",rbRemoteProxyHealth:"远端代理入口",rbLocalCcHealth:"CC Switch",rbRemoteCcHealth:"远端 CC Switch",rbHealthy:"可用",rbPending:"待验证",rbNotConfigured:"未配置",rbDisconnectDetails:"断开后 SSH Reverse Forward 会立即关闭，远端 Loopback 端口将不可访问。Codex / Claude 配置不会自动删除，需要单独使用“恢复配置”。",
  rbCcError:"未在此端口检测到 CC Switch 本地路由。请在 CC Switch 中开启 Routing Service，并检查本地路由端口后重试。",rbBridgeUnavailableError:"当前桥接不可用。请重新检查 SSH 目标并建立桥接后再执行此操作。",rbNoCapabilityError:"请至少选择一种桥接能力后继续。",rbAlreadyConnectedError:"已有桥接正在运行。请先断开，再建立新的桥接。",rbStateError:"ProxyEnv 无法读取桥接状态。没有修改任何设置；请重新打开页面后重试。",rbProcessError:"无法启动或监控 SSH 进程。请确认已安装 Windows OpenSSH 后重试。",rbRemoteError:"远端辅助程序未返回有效结果。请检查远端 Linux 账户与必需工具后重试。",rbNetworkError:"远端代理测试无法访问测试目标。隧道未被修改；请检查本机代理与远端网络策略。",rbTargetError:"已识别该远程目标，但当前配置无法安全桥接。",rbPortAllocationError:"无法找到两个可用的远端 Loopback 端口。请刷新远程环境后重试。",
};
const flowJa: FlowLabels = {
  rbClaudeOnboarding:"~/.claude.json の初回オンボーディングだけを完了済みにします。プロジェクトは事前に信頼せず、起動後にフォルダーの信頼確認を表示してから会話を開始します。",
  navLocal:"ローカル環境",navRemote:"リモートブリッジ",rbPageIntro:"SSH 環境を選び、必要な機能だけをブリッジして、確認済みの手順で利用します。",rbTargetSource:"ソース",rbTargetConfig:"設定元",rbSourceOpenSsh:"ローカル OpenSSH",rbSourceVscode:"VS Code Remote",rbSourceMoba:"MobaXterm",rbTargetUnsupported:"検出済み・現在ブリッジ不可",rbMobaUnsupported:"この MobaXterm セッションは OpenSSH に安全に変換できません。認証や踏み台設定を推測しません。",rbRegeneratePorts:"ポートを再生成",rbPortsGenerated:"利用可能なリモート Loopback ポートを生成しました",rbRemotePortAuto:"SSH 確認後に自動生成し、接続直前に再確認します。",rbPortRace:"接続前にリモートポートが使用されました。新しいポートを生成したので、確認して再接続してください。",rbCcConfirmed:"CC Switch ルーティングを検出",rbCcUnknown:"ポートは待機中ですが、サービスを確認できません",rbCcMissing:"CC Switch ルーティングを検出できません",rbCcOpenHint:"CC Switch → 設定 → 詳細 → Routing Service で Codex / Claude Code Routing を有効にして再確認してください。",rbConnectedTitle:"ブリッジを確立しました",rbNextSteps:"次の操作",rbProxyUseTitle:"通常のネットワークプロキシ",rbCcUseTitle:"CC Switch AI ルーティング",rbConfigureBeforeLaunch:"先にこの CLI を設定して検証してください。完了後に起動コマンドを使用できます。",rbStepTerminal:"リモートサーバーでターミナルを開きます。",rbStepCopy:"下のプロキシ環境変数をコピーします。",rbStepPaste:"現在の Shell に貼り付けて実行します。",rbStepTest:"ローカルプロキシ経由の接続をテストします。",rbStepRun:"同じ Shell でネットワークが必要な CLI を実行します。",rbShellScope:"環境変数は現在の Shell と子プロセスだけに有効です。.bashrc や .profile は変更しません。",rbSshHealth:"SSH 接続",rbLocalProxyHealth:"ローカルプロキシ",rbRemoteProxyHealth:"リモートプロキシ入口",rbLocalCcHealth:"CC Switch",rbRemoteCcHealth:"リモート CC Switch",rbHealthy:"利用可能",rbPending:"確認待ち",rbNotConfigured:"未設定",rbDisconnectDetails:"切断すると SSH Reverse Forward は直ちに終了し、リモート Loopback ポートは利用できなくなります。Codex / Claude 設定は残り、別途復元が必要です。",rbCcError:"このポートで CC Switch ルーティングを検出できません。Routing Service とローカルポートを確認して再試行してください。",rbBridgeUnavailableError:"ブリッジは現在利用できません。SSH 接続を再確認してから実行してください。",rbNoCapabilityError:"少なくとも 1 つの機能を選択してください。",rbAlreadyConnectedError:"既にブリッジが接続中です。切断してから新規接続してください。",rbStateError:"ブリッジ状態を読み取れません。設定は変更されていません。ページを開き直して再試行してください。",rbProcessError:"SSH プロセスを開始または監視できません。Windows OpenSSH を確認してください。",rbRemoteError:"リモートヘルパーから有効な結果が返りません。Linux アカウントと必要ツールを確認してください。",rbNetworkError:"リモート接続テストに失敗しました。トンネルは変更していません。プロキシとネットワークポリシーを確認してください。",rbTargetError:"対象は検出されましたが、現在の設定では安全にブリッジできません。",rbPortAllocationError:"利用可能な 2 つのリモート Loopback ポートを取得できません。更新して再試行してください。",
};
const flowKo: FlowLabels = {
  rbClaudeOnboarding:"~/.claude.json에서 Claude Code 최초 온보딩만 완료로 표시합니다. 프로젝트를 미리 신뢰하지 않으므로 시작 후 폴더 신뢰 확인을 거쳐 대화를 시작합니다.",
  navLocal:"로컬 환경",navRemote:"원격 브리지",rbPageIntro:"SSH 환경을 선택하고 필요한 기능만 브리지한 뒤 확인된 다음 단계를 따르세요.",rbTargetSource:"출처",rbTargetConfig:"설정 출처",rbSourceOpenSsh:"로컬 OpenSSH",rbSourceVscode:"VS Code Remote",rbSourceMoba:"MobaXterm",rbTargetUnsupported:"감지됨 · 현재 브리지 불가",rbMobaUnsupported:"이 MobaXterm 세션은 OpenSSH로 안전하게 변환할 수 없습니다. 인증이나 점프 호스트 설정을 추측하지 않습니다.",rbRegeneratePorts:"포트 다시 생성",rbPortsGenerated:"사용 가능한 원격 Loopback 포트를 생성했습니다",rbRemotePortAuto:"SSH 확인 후 자동 생성하고 연결 직전에 다시 확인합니다.",rbPortRace:"연결 전에 원격 포트가 사용되었습니다. 새 포트를 생성했으니 검토 후 다시 연결하세요.",rbCcConfirmed:"CC Switch 라우팅 감지됨",rbCcUnknown:"포트가 수신 중이지만 서비스 신원을 확인할 수 없음",rbCcMissing:"CC Switch 라우팅을 감지하지 못함",rbCcOpenHint:"CC Switch → 설정 → 고급 → Routing Service에서 Codex / Claude Code Routing을 켠 뒤 다시 감지하세요.",rbConnectedTitle:"브리지가 연결되었습니다",rbNextSteps:"다음 사용 방법",rbProxyUseTitle:"일반 네트워크 프록시",rbCcUseTitle:"CC Switch AI 라우팅",rbConfigureBeforeLaunch:"먼저 이 CLI를 설정하고 검증하세요. 완료된 후 실행 명령을 사용할 수 있습니다.",rbStepTerminal:"원격 서버에서 터미널을 여세요.",rbStepCopy:"아래 프록시 환경 변수를 복사하세요.",rbStepPaste:"현재 Shell에 붙여 넣어 실행하세요.",rbStepTest:"로컬 프록시를 통한 원격 연결을 테스트하세요.",rbStepRun:"같은 Shell에서 네트워크가 필요한 CLI를 실행하세요.",rbShellScope:"환경 변수는 현재 Shell과 하위 프로세스에만 적용됩니다. .bashrc, .profile 등은 수정하지 않습니다.",rbSshHealth:"SSH 연결",rbLocalProxyHealth:"로컬 프록시",rbRemoteProxyHealth:"원격 프록시 입구",rbLocalCcHealth:"CC Switch",rbRemoteCcHealth:"원격 CC Switch",rbHealthy:"사용 가능",rbPending:"검증 대기",rbNotConfigured:"미설정",rbDisconnectDetails:"연결을 끊으면 SSH Reverse Forward가 즉시 종료되어 원격 Loopback 포트를 사용할 수 없습니다. Codex / Claude 설정은 유지되며 별도로 복원해야 합니다.",rbCcError:"이 포트에서 CC Switch 라우팅을 감지하지 못했습니다. Routing Service와 로컬 포트를 확인한 뒤 다시 시도하세요.",rbBridgeUnavailableError:"현재 브리지를 사용할 수 없습니다. SSH 대상을 다시 확인하고 브리지를 연결한 뒤 실행하세요.",rbNoCapabilityError:"하나 이상의 브리지 기능을 선택하세요.",rbAlreadyConnectedError:"이미 브리지가 연결되어 있습니다. 먼저 연결을 끊으세요.",rbStateError:"브리지 상태를 읽을 수 없습니다. 설정은 변경되지 않았습니다. 페이지를 다시 열고 시도하세요.",rbProcessError:"SSH 프로세스를 시작하거나 감시할 수 없습니다. Windows OpenSSH 설치를 확인하세요.",rbRemoteError:"원격 도우미가 유효한 결과를 반환하지 않았습니다. Linux 계정과 필수 도구를 확인하세요.",rbNetworkError:"원격 프록시 테스트에 실패했습니다. 터널은 변경되지 않았습니다. 로컬 프록시와 네트워크 정책을 확인하세요.",rbTargetError:"대상을 감지했지만 현재 설정으로는 안전하게 브리지할 수 없습니다.",rbPortAllocationError:"사용 가능한 원격 Loopback 포트 두 개를 찾지 못했습니다. 새로 고침 후 다시 시도하세요.",
};
const foundationEn = {
  rbNetworkSection: "Network", rbNetworkSectionHint: "Server internet and local proxy bridging are checked separately. Either path may be available without the other.",
  rbAiRouteSection: "AI routing", rbAiRouteSectionHint: "CC Switch routes model requests and is independent from the general network proxy.",
  rbRefreshStatus: "Recheck status", rbLastChecked: "Last checked",
  rbCheckIdle: "Not checked", rbCheckChecking: "Checking", rbCheckHealthy: "Available", rbCheckWarning: "Needs confirmation", rbCheckFailed: "Unavailable", rbCheckDisabled: "Not enabled",
  rbServerInternet: "Server internet", rbServerInternetReachable: "Direct internet is available", rbServerInternetUnreachable: "Direct internet is unavailable", rbServerInternetUnknown: "Could not determine direct internet access",
  rbLocalProxyReady: "Local proxy is available", rbLocalProxyUnavailable: "No available local proxy", rbCcDisabled: "Not selected", rbUseProxyBridge: "Bridge this proxy", rbUseCcBridge: "Bridge AI route",
  rbHelpLabel: "About this check", rbHelpCheck: "What is checked", rbHelpSuccess: "When it succeeds", rbHelpFailure: "When it fails", rbHelpNext: "What to do next",
  rbSshHelpCheck: "Whether the selected SSH configuration can connect to the remote account.", rbSshHelpSuccess: "SSH is available; this does not prove the server can access the internet.", rbSshHelpFailure: "Host-key confirmation, authentication, configuration, or server policy may be blocking the connection.", rbSshHelpNext: "Open the target in a terminal, resolve the SSH prompt or configuration, then recheck.",
  rbInternetHelpCheck: "Whether the server can directly reach a small HTTPS connectivity endpoint without the local proxy bridge.", rbInternetHelpSuccess: "The server has its own internet path; a local proxy bridge may not be necessary.", rbInternetHelpFailure: "The server cannot reach the endpoint directly, or curl is unavailable so the result is unknown.", rbInternetHelpNext: "Use the local proxy bridge when needed, or ask the server administrator about outbound access.",
  rbProxyHelpCheck: "Whether the session-wide active proxy is still listening on this computer.", rbProxyHelpSuccess: "A general network proxy bridge can be created from this exact active proxy.", rbProxyHelpFailure: "No usable active proxy is selected, or its listening endpoint disappeared.", rbProxyHelpNext: "Return to Local environment, select an available active proxy, then recheck.",
  rbCcHelpCheck: "Whether the configured local CC Switch routing port is listening and, when possible, owned by CC Switch.", rbCcHelpSuccess: "The AI route can be bridged independently from the general network proxy.", rbCcHelpFailure: "Routing Service is off, the port is wrong, or the listening process cannot be confirmed.", rbCcHelpNext: "Enable Routing Service in CC Switch, confirm its port, then recheck.",
};
type FoundationLabels = { [K in keyof typeof foundationEn]: string };
const foundationZh: FoundationLabels = {
  rbNetworkSection:"网络",rbNetworkSectionHint:"服务器自身联网与本机代理桥接分别检测；其中一条可用，不代表另一条也可用。",
  rbAiRouteSection:"AI 路由",rbAiRouteSectionHint:"CC Switch 只负责模型请求路由，与普通网络代理相互独立。",
  rbRefreshStatus:"重新检测",rbLastChecked:"最后检查",
  rbCheckIdle:"未检查",rbCheckChecking:"检测中",rbCheckHealthy:"可用",rbCheckWarning:"待确认",rbCheckFailed:"不可用",rbCheckDisabled:"未启用",
  rbServerInternet:"服务器互联网",rbServerInternetReachable:"服务器可直接访问互联网",rbServerInternetUnreachable:"服务器无法直接访问互联网",rbServerInternetUnknown:"无法判断服务器直连状态",
  rbLocalProxyReady:"本机代理可用",rbLocalProxyUnavailable:"没有可用的本机代理",rbCcDisabled:"未选择",rbUseProxyBridge:"桥接此代理",rbUseCcBridge:"桥接 AI 路由",
  rbHelpLabel:"查看检测说明",rbHelpCheck:"检查什么",rbHelpSuccess:"成功代表什么",rbHelpFailure:"失败代表什么",rbHelpNext:"下一步怎么做",
  rbSshHelpCheck:"检查所选 SSH 配置能否连接远端账户。",rbSshHelpSuccess:"SSH 可用，但不代表服务器已经能够访问互联网。",rbSshHelpFailure:"主机指纹、认证、配置或服务器策略可能阻止连接。",rbSshHelpNext:"先在终端连接目标，处理 SSH 提示或配置后重新检测。",
  rbInternetHelpCheck:"不经过本机代理桥接，检查服务器能否直接访问一个小型 HTTPS 连通性端点。",rbInternetHelpSuccess:"服务器具备自己的互联网路径，可能无需启用本机代理桥接。",rbInternetHelpFailure:"服务器无法直接访问测试端点；缺少 curl 时结果为无法判断。",rbInternetHelpNext:"按需启用本机代理桥接，或向服务器管理员确认出站访问策略。",
  rbProxyHelpCheck:"检查本次会话的当前活动代理是否仍在本机监听。",rbProxyHelpSuccess:"可以从这个确定的活动代理建立普通网络桥接。",rbProxyHelpFailure:"尚未选择可用活动代理，或对应监听端口已经消失。",rbProxyHelpNext:"返回本机环境选择可用代理，然后重新检测。",
  rbCcHelpCheck:"检查所填 CC Switch 本地路由端口是否监听，并在可能时确认其进程身份。",rbCcHelpSuccess:"AI 路由可以独立于普通网络代理建立桥接。",rbCcHelpFailure:"Routing Service 未开启、端口不正确，或无法确认监听进程。",rbCcHelpNext:"在 CC Switch 开启 Routing Service，确认端口后重新检测。",
};
const foundationJa: FoundationLabels = {
  rbNetworkSection:"ネットワーク",rbNetworkSectionHint:"サーバー自身の接続とローカルプロキシのブリッジを別々に確認します。片方の成功はもう片方を保証しません。",
  rbAiRouteSection:"AI ルーティング",rbAiRouteSectionHint:"CC Switch はモデル要求を処理し、一般ネットワークプロキシとは独立しています。",
  rbRefreshStatus:"再確認",rbLastChecked:"最終確認",rbCheckIdle:"未確認",rbCheckChecking:"確認中",rbCheckHealthy:"利用可能",rbCheckWarning:"要確認",rbCheckFailed:"利用不可",rbCheckDisabled:"未有効",
  rbServerInternet:"サーバーインターネット",rbServerInternetReachable:"直接接続を利用できます",rbServerInternetUnreachable:"直接接続を利用できません",rbServerInternetUnknown:"直接接続を判定できません",rbLocalProxyReady:"ローカルプロキシを利用できます",rbLocalProxyUnavailable:"利用可能なローカルプロキシがありません",rbCcDisabled:"未選択",rbUseProxyBridge:"このプロキシをブリッジ",rbUseCcBridge:"AI ルートをブリッジ",
  rbHelpLabel:"確認内容の説明",rbHelpCheck:"確認する内容",rbHelpSuccess:"成功の意味",rbHelpFailure:"失敗の意味",rbHelpNext:"次の操作",
  rbSshHelpCheck:"選択した SSH 設定でリモートアカウントへ接続できるか確認します。",rbSshHelpSuccess:"SSH は利用できますが、サーバーのインターネット接続は保証しません。",rbSshHelpFailure:"ホスト鍵、認証、設定、またはサーバーポリシーが接続を妨げている可能性があります。",rbSshHelpNext:"ターミナルで対象へ接続し、SSH の確認や設定を解決してから再確認します。",
  rbInternetHelpCheck:"ローカルブリッジを使わず、サーバーから小さな HTTPS 接続先へ到達できるか確認します。",rbInternetHelpSuccess:"サーバー自身の経路があり、ローカルプロキシが不要な場合があります。",rbInternetHelpFailure:"直接到達できないか、curl がなく判定できません。",rbInternetHelpNext:"必要ならローカルプロキシを使うか、管理者に外向き通信を確認します。",
  rbProxyHelpCheck:"セッションで選択したプロキシがこの PC で待ち受けているか確認します。",rbProxyHelpSuccess:"このプロキシから一般ネットワークブリッジを作成できます。",rbProxyHelpFailure:"利用可能なプロキシが未選択、または待受ポートが消えています。",rbProxyHelpNext:"ローカル環境で利用可能なプロキシを選び、再確認します。",
  rbCcHelpCheck:"CC Switch のルーティングポートが待ち受け、可能ならプロセスも確認します。",rbCcHelpSuccess:"一般プロキシとは別に AI ルートをブリッジできます。",rbCcHelpFailure:"Routing Service が無効、ポートが不正、またはプロセスを確認できません。",rbCcHelpNext:"CC Switch の Routing Service とポートを確認して再実行します。",
};
const foundationKo: FoundationLabels = {
  rbNetworkSection:"네트워크",rbNetworkSectionHint:"서버 자체 인터넷과 로컬 프록시 브리지를 별도로 확인합니다. 한 경로의 성공이 다른 경로를 보장하지 않습니다.",
  rbAiRouteSection:"AI 라우팅",rbAiRouteSectionHint:"CC Switch 모델 라우팅은 일반 네트워크 프록시와 독립적입니다.",
  rbRefreshStatus:"다시 확인",rbLastChecked:"마지막 확인",rbCheckIdle:"확인 안 함",rbCheckChecking:"확인 중",rbCheckHealthy:"사용 가능",rbCheckWarning:"확인 필요",rbCheckFailed:"사용 불가",rbCheckDisabled:"사용 안 함",
  rbServerInternet:"서버 인터넷",rbServerInternetReachable:"직접 인터넷 사용 가능",rbServerInternetUnreachable:"직접 인터넷 사용 불가",rbServerInternetUnknown:"직접 인터넷 상태를 판단할 수 없음",rbLocalProxyReady:"로컬 프록시 사용 가능",rbLocalProxyUnavailable:"사용 가능한 로컬 프록시 없음",rbCcDisabled:"선택 안 함",rbUseProxyBridge:"이 프록시 브리지",rbUseCcBridge:"AI 라우트 브리지",
  rbHelpLabel:"검사 설명",rbHelpCheck:"검사 항목",rbHelpSuccess:"성공 의미",rbHelpFailure:"실패 의미",rbHelpNext:"다음 단계",
  rbSshHelpCheck:"선택한 SSH 설정으로 원격 계정에 연결할 수 있는지 확인합니다.",rbSshHelpSuccess:"SSH를 사용할 수 있지만 서버 인터넷 연결을 보장하지 않습니다.",rbSshHelpFailure:"호스트 키, 인증, 설정 또는 서버 정책이 연결을 막을 수 있습니다.",rbSshHelpNext:"터미널에서 대상에 연결해 SSH 확인이나 설정을 해결한 뒤 다시 검사하세요.",
  rbInternetHelpCheck:"로컬 브리지 없이 서버가 작은 HTTPS 연결 대상에 직접 도달하는지 확인합니다.",rbInternetHelpSuccess:"서버 자체 인터넷 경로가 있어 로컬 프록시가 필요하지 않을 수 있습니다.",rbInternetHelpFailure:"직접 연결할 수 없거나 curl이 없어 판단할 수 없습니다.",rbInternetHelpNext:"필요하면 로컬 프록시 브리지를 사용하거나 관리자에게 외부 연결 정책을 문의하세요.",
  rbProxyHelpCheck:"세션의 현재 활성 프록시가 이 컴퓨터에서 계속 수신 중인지 확인합니다.",rbProxyHelpSuccess:"이 활성 프록시로 일반 네트워크 브리지를 만들 수 있습니다.",rbProxyHelpFailure:"사용 가능한 프록시가 선택되지 않았거나 수신 포트가 사라졌습니다.",rbProxyHelpNext:"로컬 환경에서 사용 가능한 프록시를 선택한 뒤 다시 검사하세요.",
  rbCcHelpCheck:"CC Switch 라우팅 포트가 수신 중인지, 가능한 경우 프로세스도 확인합니다.",rbCcHelpSuccess:"일반 프록시와 별도로 AI 라우트를 브리지할 수 있습니다.",rbCcHelpFailure:"Routing Service가 꺼졌거나 포트가 틀렸거나 프로세스를 확인할 수 없습니다.",rbCcHelpNext:"CC Switch에서 Routing Service와 포트를 확인한 뒤 다시 검사하세요.",
};
const authEn = {
  rbAuthTitle: "OpenSSH authentication", rbAuthPrompt: "OpenSSH prompt", rbAuthCancel: "Cancel authentication",
  rbAuthWaitingPrompt: "Waiting for server", rbAuthWaitingUser: "Waiting for input", rbAuthVerifying: "Verifying", rbAuthCompleting: "Completing remote check", rbAuthCompletingTitle: "Signed in, checking the remote environment", rbAuthCompletingDescription: "OpenSSH authentication succeeded. ProxyEnv is waiting for the remote check result.", rbAuthRemoteCheckFailure: "Remote check failed", rbAuthRemoteCheckFailureTitle: "The remote check did not complete", rbAuthSuccess: "SSH authentication succeeded", rbAuthFailure: "Authentication failed",
  rbAuthWaitingPromptMessage: "Waiting for the server authentication request…", rbAuthInteractionError: "Interaction error", rbAuthPromptUnavailableTitle: "Could not read the authentication prompt", rbAuthPromptUnavailableDescription: "ProxyEnv started OpenSSH but did not receive a recognizable authentication request.", rbAuthPromptUnavailableHint: "This usually indicates a local OpenSSH or ConPTY interaction problem. It does not mean your server password is wrong.",
  rbAuthRetry: "Try again", rbAuthOpenDiagnostic: "Open diagnostics", rbAuthHideDiagnostic: "Hide diagnostics", rbAuthDiagnosticBytes: "PTY bytes received", rbAuthDiagnosticPrintable: "Printable bytes", rbAuthDiagnosticCpr: "Cursor queries", rbAuthDiagnosticPrompt: "Prompt detected", rbAuthDiagnosticMarker: "Authentication marker", rbAuthDiagnosticResult: "Remote result detected", rbAuthDiagnosticClosed: "PTY output closed", rbAuthDiagnosticYes: "Yes", rbAuthDiagnosticNo: "No", rbAuthCompletionTimeout: "Authentication succeeded, but the remote check did not return a result in time. Open diagnostics and retry.",
  rbAuthPasswordTitle: "SSH password authentication", rbAuthPasswordDescription: "This server requires your account password to complete SSH sign-in.", rbAuthPasswordLabel: "Server password",
  rbAuthPassphraseTitle: "SSH key passphrase", rbAuthPassphraseDescription: "The selected SSH private key is protected. Enter its passphrase to unlock it for this session.", rbAuthPassphraseLabel: "Key passphrase",
  rbAuthOtpTitle: "Verification code", rbAuthOtpDescription: "The server requires an additional identity verification step.", rbAuthOtpLabel: "Verification code",
  rbAuthHostKeyTitle: "Confirm first connection", rbAuthHostKeyDescription: "OpenSSH has not seen this server key before. Confirm the fingerprint belongs to the server you intend to use.", rbAuthFingerprint: "Host fingerprint",
  rbAuthKeyboardTitle: "Additional server authentication", rbAuthKeyboardDescription: "The server is asking an additional authentication question.", rbAuthUnknownTitle: "Your confirmation is required", rbAuthUnknownDescription: "OpenSSH needs more information to continue.", rbAuthResponseLabel: "Response",
  rbAuthSecretNotice: "This value is sent only to the current OpenSSH session. ProxyEnv does not save it in commands, configuration, or logs.", rbAuthPassphraseNotice: "This is not your server account password. It is used only to unlock the SSH key for this OpenSSH session.", rbAuthGenericNotice: "This response is sent only to the current OpenSSH session.",
  rbAuthConnect: "Connect", rbAuthContinue: "Continue", rbAuthVerify: "Verify", rbAuthConfirmHost: "Confirm and continue", rbAuthSubmit: "Submit",
  rbAuthRejected: "The server rejected that response. Check it and try again.", rbAuthHostKeyChanged: "The server key differs from the existing known_hosts record. ProxyEnv will not bypass or overwrite it.",
  rbAuthMethodIdentityFile: "SSH key", rbAuthMethodAgent: "SSH agent", rbAuthMethodPassword: "Password", rbAuthMethodKeyboardInteractive: "Interactive challenge", rbAuthMethodUnknown: "OpenSSH authentication",
  rbAuthError: "Interactive SSH authentication could not be completed. Check the prompt, account policy, and host key, then try again.",
};
type AuthLabels = { [K in keyof typeof authEn]: string };
const authZh: AuthLabels = {
  rbAuthTitle:"OpenSSH 认证",rbAuthPrompt:"OpenSSH 提示",rbAuthCancel:"取消认证",
  rbAuthWaitingPrompt:"等待服务器",rbAuthWaitingUser:"等待输入",rbAuthVerifying:"正在验证",rbAuthCompleting:"正在完成远端检查",rbAuthCompletingTitle:"认证通过，正在检查远端环境",rbAuthCompletingDescription:"OpenSSH 已完成登录，ProxyEnv 正在等待远端检查结果。",rbAuthRemoteCheckFailure:"远端检查失败",rbAuthRemoteCheckFailureTitle:"远端检查未完成",rbAuthSuccess:"SSH 认证成功",rbAuthFailure:"认证失败",rbAuthWaitingPromptMessage:"正在等待服务器的认证请求……",rbAuthInteractionError:"交互异常",rbAuthPromptUnavailableTitle:"未能读取认证提示",rbAuthPromptUnavailableDescription:"ProxyEnv 已启动 OpenSSH，但没有收到可以识别的认证请求。",rbAuthPromptUnavailableHint:"这通常属于本机 OpenSSH 或 ConPTY 交互异常，并不表示服务器密码错误。",
  rbAuthRetry:"重新尝试",rbAuthOpenDiagnostic:"打开诊断",rbAuthHideDiagnostic:"收起诊断",rbAuthDiagnosticBytes:"PTY 接收字节",rbAuthDiagnosticPrintable:"可显示字节",rbAuthDiagnosticCpr:"光标查询",rbAuthDiagnosticPrompt:"识别到提示",rbAuthDiagnosticMarker:"认证标记",rbAuthDiagnosticResult:"识别到远端结果",rbAuthDiagnosticClosed:"PTY 输出已关闭",rbAuthDiagnosticYes:"是",rbAuthDiagnosticNo:"否",rbAuthCompletionTimeout:"SSH 认证已通过，但远端检查未在限定时间内返回结果。请打开诊断后重试。",
  rbAuthPasswordTitle:"SSH 密码认证",rbAuthPasswordDescription:"此服务器要求使用账户密码完成 SSH 登录。",rbAuthPasswordLabel:"服务器密码",
  rbAuthPassphraseTitle:"SSH 密钥密码",rbAuthPassphraseDescription:"当前 SSH 私钥受到密码保护，请输入密码以在本次会话中解锁。",rbAuthPassphraseLabel:"密钥密码",
  rbAuthOtpTitle:"身份验证码",rbAuthOtpDescription:"服务器要求进一步验证你的身份。",rbAuthOtpLabel:"验证码",
  rbAuthHostKeyTitle:"首次连接确认",rbAuthHostKeyDescription:"OpenSSH 尚未记录这台服务器的主机密钥，请确认指纹属于你要连接的服务器。",rbAuthFingerprint:"主机指纹",
  rbAuthKeyboardTitle:"服务器附加认证",rbAuthKeyboardDescription:"服务器要求回答额外的身份认证问题。",rbAuthUnknownTitle:"需要你的确认",rbAuthUnknownDescription:"OpenSSH 需要更多信息才能继续。",rbAuthResponseLabel:"响应",
  rbAuthSecretNotice:"此内容只会发送给当前 OpenSSH 会话。ProxyEnv 不会保存到命令、配置或日志。",rbAuthPassphraseNotice:"这不是服务器账户密码，只用于在本次 OpenSSH 会话中解锁 SSH 密钥。",rbAuthGenericNotice:"此响应只会发送给当前 OpenSSH 会话。",
  rbAuthConnect:"连接",rbAuthContinue:"继续",rbAuthVerify:"验证",rbAuthConfirmHost:"确认并继续",rbAuthSubmit:"提交",rbAuthRejected:"服务器拒绝了本次认证，请检查后重新输入。",rbAuthHostKeyChanged:"服务器主机密钥与 known_hosts 中的记录不同。ProxyEnv 不会绕过或覆盖已有指纹。",
  rbAuthMethodIdentityFile:"SSH 密钥",rbAuthMethodAgent:"SSH Agent",rbAuthMethodPassword:"密码",rbAuthMethodKeyboardInteractive:"交互式验证",rbAuthMethodUnknown:"OpenSSH 认证",
  rbAuthError:"未能完成 SSH 交互认证。请检查提示内容、账户策略与主机指纹后重试。",
};
const authJa: AuthLabels = {
  rbAuthTitle:"OpenSSH 認証",rbAuthPrompt:"OpenSSH の確認",rbAuthCancel:"認証をキャンセル",
  rbAuthWaitingPrompt:"サーバーを待機",rbAuthWaitingUser:"入力待ち",rbAuthVerifying:"確認中",rbAuthCompleting:"リモート確認を完了中",rbAuthCompletingTitle:"認証済み、リモート環境を確認中",rbAuthCompletingDescription:"OpenSSH の認証が完了しました。ProxyEnv はリモート確認結果を待っています。",rbAuthRemoteCheckFailure:"リモート確認に失敗",rbAuthRemoteCheckFailureTitle:"リモート確認を完了できませんでした",rbAuthSuccess:"SSH 認証に成功しました",rbAuthFailure:"認証に失敗しました",rbAuthWaitingPromptMessage:"サーバーの認証要求を待っています…",rbAuthInteractionError:"対話エラー",rbAuthPromptUnavailableTitle:"認証プロンプトを読み取れませんでした",rbAuthPromptUnavailableDescription:"ProxyEnv は OpenSSH を起動しましたが、識別可能な認証要求を受信できませんでした。",rbAuthPromptUnavailableHint:"通常はローカル OpenSSH または ConPTY の対話問題であり、サーバーパスワードの誤りを意味しません。",
  rbAuthRetry:"再試行",rbAuthOpenDiagnostic:"診断を開く",rbAuthHideDiagnostic:"診断を閉じる",rbAuthDiagnosticBytes:"PTY 受信バイト",rbAuthDiagnosticPrintable:"表示可能バイト",rbAuthDiagnosticCpr:"カーソル照会",rbAuthDiagnosticPrompt:"プロンプト検出",rbAuthDiagnosticMarker:"認証マーカー",rbAuthDiagnosticResult:"リモート結果を検出",rbAuthDiagnosticClosed:"PTY 出力終了",rbAuthDiagnosticYes:"はい",rbAuthDiagnosticNo:"いいえ",rbAuthCompletionTimeout:"SSH 認証は成功しましたが、リモート確認結果が時間内に返りませんでした。診断を開いて再試行してください。",
  rbAuthPasswordTitle:"SSH パスワード認証",rbAuthPasswordDescription:"このサーバーではアカウントのパスワードによる SSH ログインが必要です。",rbAuthPasswordLabel:"サーバーパスワード",
  rbAuthPassphraseTitle:"SSH キーのパスフレーズ",rbAuthPassphraseDescription:"選択した SSH 秘密鍵は保護されています。このセッションで解除するパスフレーズを入力してください。",rbAuthPassphraseLabel:"キーパスフレーズ",
  rbAuthOtpTitle:"確認コード",rbAuthOtpDescription:"サーバーが追加の本人確認を要求しています。",rbAuthOtpLabel:"確認コード",
  rbAuthHostKeyTitle:"初回接続の確認",rbAuthHostKeyDescription:"OpenSSH に未登録のホスト鍵です。接続先サーバーの指紋であることを確認してください。",rbAuthFingerprint:"ホスト指紋",
  rbAuthKeyboardTitle:"サーバーの追加認証",rbAuthKeyboardDescription:"サーバーが追加の認証質問を要求しています。",rbAuthUnknownTitle:"確認が必要です",rbAuthUnknownDescription:"OpenSSH を続行するには追加情報が必要です。",rbAuthResponseLabel:"応答",
  rbAuthSecretNotice:"この内容は現在の OpenSSH セッションだけに送信され、コマンド・設定・ログには保存されません。",rbAuthPassphraseNotice:"これはサーバーのアカウントパスワードではありません。現在のセッションで SSH キーを解除するためだけに使用します。",rbAuthGenericNotice:"この応答は現在の OpenSSH セッションだけに送信されます。",
  rbAuthConnect:"接続",rbAuthContinue:"続行",rbAuthVerify:"確認",rbAuthConfirmHost:"確認して続行",rbAuthSubmit:"送信",rbAuthRejected:"サーバーがこの認証を拒否しました。確認して再入力してください。",rbAuthHostKeyChanged:"サーバー鍵が known_hosts の記録と異なります。ProxyEnv は既存の指紋を回避または上書きしません。",
  rbAuthMethodIdentityFile:"SSH キー",rbAuthMethodAgent:"SSH Agent",rbAuthMethodPassword:"パスワード",rbAuthMethodKeyboardInteractive:"対話チャレンジ",rbAuthMethodUnknown:"OpenSSH 認証",
  rbAuthError:"SSH 対話認証を完了できませんでした。表示内容、アカウントポリシー、ホスト鍵を確認して再試行してください。",
};
const authKo: AuthLabels = {
  rbAuthTitle:"OpenSSH 인증",rbAuthPrompt:"OpenSSH 요청",rbAuthCancel:"인증 취소",
  rbAuthWaitingPrompt:"서버 대기 중",rbAuthWaitingUser:"입력 대기",rbAuthVerifying:"확인 중",rbAuthCompleting:"원격 확인 완료 중",rbAuthCompletingTitle:"인증됨, 원격 환경 확인 중",rbAuthCompletingDescription:"OpenSSH 인증이 완료되었습니다. ProxyEnv가 원격 확인 결과를 기다리고 있습니다.",rbAuthRemoteCheckFailure:"원격 확인 실패",rbAuthRemoteCheckFailureTitle:"원격 확인을 완료하지 못했습니다",rbAuthSuccess:"SSH 인증 성공",rbAuthFailure:"인증 실패",rbAuthWaitingPromptMessage:"서버의 인증 요청을 기다리는 중…",rbAuthInteractionError:"상호 작용 오류",rbAuthPromptUnavailableTitle:"인증 요청을 읽지 못했습니다",rbAuthPromptUnavailableDescription:"ProxyEnv가 OpenSSH를 시작했지만 인식 가능한 인증 요청을 받지 못했습니다.",rbAuthPromptUnavailableHint:"일반적으로 로컬 OpenSSH 또는 ConPTY 상호 작용 문제이며 서버 비밀번호가 틀렸다는 의미는 아닙니다.",
  rbAuthRetry:"다시 시도",rbAuthOpenDiagnostic:"진단 열기",rbAuthHideDiagnostic:"진단 닫기",rbAuthDiagnosticBytes:"PTY 수신 바이트",rbAuthDiagnosticPrintable:"표시 가능 바이트",rbAuthDiagnosticCpr:"커서 조회",rbAuthDiagnosticPrompt:"요청 감지",rbAuthDiagnosticMarker:"인증 마커",rbAuthDiagnosticResult:"원격 결과 감지",rbAuthDiagnosticClosed:"PTY 출력 종료",rbAuthDiagnosticYes:"예",rbAuthDiagnosticNo:"아니요",rbAuthCompletionTimeout:"SSH 인증은 성공했지만 원격 확인 결과가 제한 시간 내에 반환되지 않았습니다. 진단을 열고 다시 시도하세요.",
  rbAuthPasswordTitle:"SSH 비밀번호 인증",rbAuthPasswordDescription:"이 서버는 계정 비밀번호로 SSH 로그인을 완료해야 합니다.",rbAuthPasswordLabel:"서버 비밀번호",
  rbAuthPassphraseTitle:"SSH 키 암호",rbAuthPassphraseDescription:"선택한 SSH 개인 키가 보호되어 있습니다. 이 세션에서 잠금을 해제할 암호를 입력하세요.",rbAuthPassphraseLabel:"키 암호",
  rbAuthOtpTitle:"인증 코드",rbAuthOtpDescription:"서버가 추가 본인 인증을 요구합니다.",rbAuthOtpLabel:"인증 코드",
  rbAuthHostKeyTitle:"첫 연결 확인",rbAuthHostKeyDescription:"OpenSSH에 아직 등록되지 않은 호스트 키입니다. 연결할 서버의 지문인지 확인하세요.",rbAuthFingerprint:"호스트 지문",
  rbAuthKeyboardTitle:"서버 추가 인증",rbAuthKeyboardDescription:"서버가 추가 인증 질문을 요청합니다.",rbAuthUnknownTitle:"확인이 필요합니다",rbAuthUnknownDescription:"OpenSSH를 계속하려면 추가 정보가 필요합니다.",rbAuthResponseLabel:"응답",
  rbAuthSecretNotice:"이 내용은 현재 OpenSSH 세션에만 전송되며 명령, 설정 또는 로그에 저장되지 않습니다.",rbAuthPassphraseNotice:"서버 계정 비밀번호가 아닙니다. 현재 OpenSSH 세션에서 SSH 키를 잠금 해제할 때만 사용합니다.",rbAuthGenericNotice:"이 응답은 현재 OpenSSH 세션에만 전송됩니다.",
  rbAuthConnect:"연결",rbAuthContinue:"계속",rbAuthVerify:"확인",rbAuthConfirmHost:"확인 후 계속",rbAuthSubmit:"제출",rbAuthRejected:"서버가 이번 인증을 거부했습니다. 확인한 뒤 다시 입력하세요.",rbAuthHostKeyChanged:"서버 키가 known_hosts 기록과 다릅니다. ProxyEnv는 기존 지문을 우회하거나 덮어쓰지 않습니다.",
  rbAuthMethodIdentityFile:"SSH 키",rbAuthMethodAgent:"SSH Agent",rbAuthMethodPassword:"비밀번호",rbAuthMethodKeyboardInteractive:"대화형 확인",rbAuthMethodUnknown:"OpenSSH 인증",
  rbAuthError:"SSH 대화형 인증을 완료하지 못했습니다. 요청 내용, 계정 정책 및 호스트 키를 확인한 뒤 다시 시도하세요.",
};
const en = {
  ...extensionEn,
  ...flowEn,
  ...foundationEn,
  ...authEn,
  rbVscodeOpen: "Open in VS Code", rbVscodeTargets: "Also reads remote.SSH.configFile from the default VS Code user settings (JSONC). Hosts from that file are marked VS Code. Custom profiles and portable settings are not scanned.", rbVscodeHint: "Connect VS Code Remote - SSH to this same host, then paste the proxy variables and CLI launch commands into its remote terminal. The bridge remains owned by ProxyEnv. Terminal exports do not reconfigure an already-running VS Code Server or extension host.", rbVscodeError: "Could not open the reviewed SSH target in VS Code. Check that VS Code and Remote - SSH are installed, and select a host from the same SSH configuration used by VS Code.",
  rbTitle: "Remote environment bridge", rbHint: "Share your current local proxy and CC Switch with a server or VM over SSH.",
  rbOpen: "Configure bridge", rbView: "View bridge", rbTarget: "Remote environment", rbCapabilities: "Bridge capabilities", rbPreview: "Review bridge", rbStatus: "Bridge status",
  rbAlias: "SSH host alias", rbEmpty: "No compatible remote environments were found in OpenSSH, VS Code Remote, or MobaXterm. Add or repair a target, then refresh.",
  rbRequirements: "Uses OpenSSH keys or agent first, then offers in-app password / keyboard-interactive authentication when required. Verify a new host key in your terminal first. Remote: a non-root Linux account with ss, flock and coreutils.",
  rbCheck: "Check connection", rbChecked: "SSH connection verified", rbRefresh: "Refresh remote environments", rbNext: "Continue", rbBack: "Back", rbClose: "Close",
  rbProxy: "Current local proxy", rbNoProxy: "The active proxy is unavailable. Select an available proxy under Local environment.",
  rbCc: "CC Switch local routing", rbCcHint: "Checks only the loopback port, not service identity. Confirm this is your CC Switch routing port and routing is enabled for the desired CLI.",
  rbLocalPort: "Local routing port", rbDetect: "Check local port", rbDetected: "Local port is listening", rbRemotePort: "Remote port", rbPortHint: "Use distinct ports between 1024 and 65535.",
  rbSafety: "Remote binding: 127.0.0.1 only. The tunnel lasts for this ProxyEnv session. No shell startup files are changed.",
  rbConnect: "Establish bridge", rbDisconnect: "Disconnect bridge", rbDisconnectHint: "Remote programs will lose bridge access. Saved CLI overlays remain available for restore.", rbConfirm: "Confirm", rbCancel: "Cancel",
  rbReconnect: "Configure again", rbStaleHint: "The active proxy has changed. This tunnel still uses its original endpoint. Disconnect and establish a new bridge to use the current proxy.",
  rbUnavailableHint: "A local endpoint is unavailable. Check the selected proxy or CC Switch. The bridge will not switch endpoints automatically.",
  rbLocal: "Local", rbRemote: "Remote", rbCopy: "Copy environment variables", rbCopied: "Copied", rbTest: "Test bridge", rbTestHint: "Only this action contacts gstatic.com through the remote proxy. No AI prompts are sent.", rbTested: "Remote network test passed",
  rbCodex: "Configure Codex", rbClaude: "Configure Claude Code", rbRestoreCodex: "Restore Codex overlay", rbRestoreClaude: "Restore Claude overlay",
  rbConfigHint: "Creates a dedicated overlay: Codex CLI 0.134+ (0.x), or Claude Code 2.x. Existing defaults and credentials stay in place. Use the displayed command to opt in; higher-priority settings may override it. PROXY_MANAGED is a public placeholder, not a key.",
  rbBefore: "Before", rbAfter: "After", rbAbsent: "File does not exist", rbApply: "Apply reviewed configuration", rbApplied: "Configuration verified", rbLaunch: "Remote launch command", rbCopyLaunch: "Copy launch command", rbRestoreHint: "Restore the original dedicated overlay from its remote backup. If another program changed the file, restore stops without overwriting it.", rbRestored: "Original overlay restored",
  rbBusy: "Working…", rbFailed: "The operation failed. Check your SSH target and local endpoints, then retry.",
  rbSshError: "SSH could not connect. Check the host key in your terminal, existing key/agent authentication, and SSH configuration. No server settings were changed.",
  rbForwardError: "The reverse forward could not be established. Check remote port occupancy and SSH forwarding policy. Server settings were not changed.",
  rbBindingError: "The remote listener could not be verified as loopback-only. The new tunnel was closed. Check GatewayPorts with your administrator.",
  rbConfigError: "Configuration conflict or unsupported file. No conflicting edits were overwritten. Review the dedicated overlay and recovery files on the remote host.",
  rbDependencyError: "Unsupported remote environment or missing tool. Use a non-root Linux account with ss, flock and coreutils; CLI overlays require Codex 0.134+ (0.x) or Claude Code 2.x.",
  rbPortError: "Invalid or occupied remote port. Choose distinct unused ports in 1024–65535.", rbActiveError: "The selected proxy changed or is unavailable. Return home to check it, then review the bridge again.",
  rbStates: { disconnected: "Disconnected", connecting: "Connecting", connected: "Connected", stale: "Configuration changed", unavailable: "Unavailable", error: "Connection failed" },
};
type Labels = { [K in keyof typeof en]: K extends "rbStates" ? Record<keyof typeof en.rbStates, string> : string };
const zh: Labels = {
  ...extensionZh,
  ...flowZh,
  ...foundationZh,
  ...authZh,
  rbVscodeOpen:"在 VS Code 中打开", rbVscodeTargets:"同时读取 VS Code 默认用户设置（JSONC）中的 remote.SSH.configFile；该文件中的主机标注为 VS Code。不扫描自定义 Profile 或便携版设置。", rbVscodeHint:"在 VS Code Remote - SSH 中连接同一主机，再把代理变量和 CLI 启动命令粘贴到远程终端。桥接由 ProxyEnv 独立维护。终端 export 不会重新配置已运行的 VS Code Server 或扩展宿主。", rbVscodeError:"无法在 VS Code 中打开已确认的 SSH 目标。请确认已安装 VS Code 与 Remote - SSH，并选择与 VS Code 使用同一 SSH 配置的主机。",
  rbTitle:"远程环境桥接", rbHint:"通过 SSH 让服务器或虚拟机复用当前本机代理与 CC Switch。", rbOpen:"配置远程桥接", rbView:"查看桥接", rbTarget:"选择远程环境", rbCapabilities:"选择桥接能力", rbPreview:"确认桥接配置", rbStatus:"桥接状态",
  rbAlias:"SSH 主机别名", rbEmpty:"未在 OpenSSH、VS Code Remote 或 MobaXterm 中发现可用的远程环境。请添加或修复目标后刷新。", rbRequirements:"优先复用 OpenSSH 密钥与 Agent；需要时可在应用内完成密码或 Keyboard Interactive 认证。新主机仍需先在终端确认指纹。远端需使用非 root Linux 账户，并具备 ss、flock 和 coreutils。",
  rbCheck:"检查连接", rbChecked:"SSH 连接已验证", rbRefresh:"刷新远程环境", rbNext:"下一步", rbBack:"上一步", rbClose:"关闭", rbProxy:"当前本机代理", rbNoProxy:"当前活动代理不可用，请回到本机环境选择可用代理。",
  rbCc:"CC Switch 本地路由", rbCcHint:"仅检查 Loopback 端口监听，不判断服务身份。请确认该端口属于 CC Switch，且已为所需 CLI 开启路由。", rbLocalPort:"本地路由端口", rbDetect:"检查本地端口", rbDetected:"本地端口正在监听", rbRemotePort:"远程端口", rbPortHint:"端口范围 1024–65535，两个桥接不能使用相同端口。",
  rbSafety:"远端仅绑定 127.0.0.1。隧道仅在本次 ProxyEnv 运行期间有效，不修改 Shell 启动文件。", rbConnect:"建立桥接", rbDisconnect:"断开桥接", rbDisconnectHint:"远程程序将无法继续通过桥接访问。已保存的 CLI 接入配置仍可恢复。", rbConfirm:"确认", rbCancel:"取消", rbReconnect:"重新配置",
  rbStaleHint:"当前活动代理已变化，现有隧道仍使用原端点。请断开并重新建立桥接以使用当前代理。", rbUnavailableHint:"本地端点不可用，请检查所选代理或 CC Switch。桥接不会自动切换端点。", rbLocal:"本机", rbRemote:"远端", rbCopy:"复制环境变量", rbCopied:"已复制", rbTest:"测试桥接", rbTestHint:"仅点击此按钮才会经远程代理访问 gstatic.com，不发送 AI Prompt。", rbTested:"远程联网测试通过",
  rbCodex:"配置 Codex", rbClaude:"配置 Claude Code", rbRestoreCodex:"恢复 Codex 接入配置", rbRestoreClaude:"恢复 Claude 接入配置", rbConfigHint:"创建独立接入配置，支持 Codex CLI 0.134+（0.x）和 Claude Code 2.x。保留原有默认配置与凭据，使用下方命令主动启用；更高优先级设置可能覆盖它。PROXY_MANAGED 是公开占位符，不是密钥。",
  rbBefore:"修改前", rbAfter:"修改后", rbAbsent:"文件不存在", rbApply:"确认应用配置", rbApplied:"配置已写入并验证", rbLaunch:"远端启动命令", rbCopyLaunch:"复制启动命令", rbRestoreHint:"从远端备份恢复原始独立接入配置。如果第三方已修改文件，将停止恢复，避免覆盖。", rbRestored:"已恢复原始接入配置", rbBusy:"正在处理…", rbFailed:"操作未完成，请检查 SSH 目标和本地端点后重试。",
  rbSshError:"SSH 无法连接。请在终端检查主机指纹、密钥或 Agent 认证，以及 SSH 配置。未修改服务器设置。", rbForwardError:"无法建立反向转发，请检查远程端口占用和 SSH 转发策略。未修改服务器设置。", rbBindingError:"无法确认远端仅监听 Loopback，新隧道已关闭。请联系管理员检查 GatewayPorts 策略。", rbConfigError:"配置冲突或文件不受支持，未覆盖冲突修改。请在远端检查独立接入配置及恢复文件。", rbDependencyError:"远端环境不受支持或缺少工具。需使用具备 ss、flock、coreutils 的非 root Linux 账户；接入配置要求 Codex 0.134+（0.x）或 Claude Code 2.x。", rbPortError:"远程端口无效或已占用，请选择 1024–65535 范围内互不重复的空闲端口。", rbActiveError:"活动代理已变化或不可用，请回首页检查后重新预览桥接。",
  rbStates:{disconnected:"未连接",connecting:"正在连接",connected:"已连接",stale:"配置已变化",unavailable:"当前不可用",error:"连接失败"},
};
const ja: Labels = {
  ...extensionJa,
  ...flowJa,
  ...foundationJa,
  ...authJa,
  rbVscodeOpen:"VS Code で開く", rbVscodeTargets:"VS Code の既定ユーザー設定（JSONC）の remote.SSH.configFile も読みます。該当ホストは VS Code と表示します。カスタム Profile とポータブル設定は対象外です。", rbVscodeHint:"Remote - SSH で同じホストに接続し、リモートターミナルに環境変数と CLI コマンドを貼り付けてください。ブリッジは ProxyEnv が管理します。export は起動済みの Server や拡張ホストを再設定しません。", rbVscodeError:"VS Code で接続先を開けません。VS Code と Remote - SSH のインストール、および SSH 設定ファイルの一致を確認してください。",
  rbTitle:"リモート環境ブリッジ",rbHint:"SSH 経由でサーバーや VM と現在のプロキシ・CC Switch を共有します。",rbOpen:"ブリッジを設定",rbView:"ブリッジを表示",rbTarget:"接続先を選択",rbCapabilities:"機能を選択",rbPreview:"設定を確認",rbStatus:"ブリッジの状態",rbAlias:"SSH ホスト別名",rbEmpty:"~/.ssh/config に明示的な Host がありません。OpenSSH で追加して更新してください。",rbRequirements:"既存の鍵と Agent を使用します。先にターミナルでホスト鍵を確認してください。接続先は非 root の Linux、ss・flock・coreutils が必要です。",rbCheck:"接続を確認",rbChecked:"SSH 接続を確認済み",rbRefresh:"ホストを更新",rbNext:"次へ",rbBack:"戻る",rbClose:"閉じる",rbProxy:"現在のローカルプロキシ",rbNoProxy:"プロキシが利用できません。ホームで利用可能なプロキシを選択してください。",rbCc:"CC Switch ローカルルーティング",rbCcHint:"ポートの待ち受けのみ確認します。CC Switch のポートと CLI のルーティング有効化を確認してください。",rbLocalPort:"ローカルポート",rbDetect:"ポートを確認",rbDetected:"ポートは待ち受け中です",rbRemotePort:"リモートポート",rbPortHint:"1024～65535 の異なるポートを指定してください。",rbSafety:"リモートは 127.0.0.1 のみ。ProxyEnv の実行中だけ有効です。Shell 起動ファイルは変更しません。",rbConnect:"ブリッジを接続",rbDisconnect:"ブリッジを切断",rbDisconnectHint:"リモートプログラムの接続が失われます。保存済み CLI 設定は復元できます。",rbConfirm:"確認",rbCancel:"キャンセル",rbReconnect:"再設定",rbStaleHint:"現在のプロキシが変更されました。既存の接続先は保持されています。切断してから再接続してください。",rbUnavailableHint:"ローカルの接続先を利用できません。プロキシまたは CC Switch を確認してください。自動切替は行いません。",rbLocal:"ローカル",rbRemote:"リモート",rbCopy:"環境変数をコピー",rbCopied:"コピーしました",rbTest:"ブリッジをテスト",rbTestHint:"この操作のみがリモートプロキシ経由で gstatic.com に接続します。AI リクエストは送りません。",rbTested:"ネットワークテスト成功",rbCodex:"Codex を設定",rbClaude:"Claude Code を設定",rbRestoreCodex:"Codex 設定を復元",rbRestoreClaude:"Claude 設定を復元",rbConfigHint:"専用設定を作成します。Codex CLI 0.134+（0.x）と Claude Code 2.x に対応。既存の設定と認証情報を保持します。表示されたコマンドで有効にしてください。上位設定による上書きがあり得ます。PROXY_MANAGED は公開プレースホルダーです。",rbBefore:"変更前",rbAfter:"変更後",rbAbsent:"ファイルなし",rbApply:"確認して適用",rbApplied:"設定を検証済み",rbLaunch:"リモート起動コマンド",rbCopyLaunch:"起動コマンドをコピー",rbRestoreHint:"リモートバックアップから専用設定を復元します。他の変更があれば上書きせず停止します。",rbRestored:"元の設定を復元しました",rbBusy:"処理中…",rbFailed:"操作に失敗しました。SSH とローカル接続先を確認して再試行してください。",rbSshError:"SSH 接続に失敗しました。ターミナルでホスト鍵、認証、設定を確認してください。サーバー設定は変更していません。",rbForwardError:"転送できません。ポートの使用状況と SSH 転送ポリシーを確認してください。サーバー設定は変更していません。",rbBindingError:"Loopback のみの待ち受けを確認できず、新しい接続を閉じました。管理者に GatewayPorts を確認してください。",rbConfigError:"設定が競合するか未対応です。競合を上書きしていません。リモートの専用設定と復元ファイルを確認してください。",rbDependencyError:"非対応の環境またはツール不足です。非 root Linux と ss・flock・coreutils、Codex 0.134+（0.x）または Claude Code 2.x が必要です。",rbPortError:"ポートが無効か使用中です。1024～65535 の空きポートを選択してください。",rbActiveError:"プロキシが変更されたか利用できません。ホームで確認して再設定してください。",rbStates:{disconnected:"未接続",connecting:"接続中",connected:"接続済み",stale:"設定変更あり",unavailable:"利用不可",error:"接続失敗"},
};
const ko: Labels = {
  ...extensionKo,
  ...flowKo,
  ...foundationKo,
  ...authKo,
  rbVscodeOpen:"VS Code에서 열기", rbVscodeTargets:"VS Code 기본 사용자 설정(JSONC)의 remote.SSH.configFile도 읽습니다. 해당 호스트는 VS Code로 표시됩니다. 사용자 지정 Profile 및 휴대용 설정은 검색하지 않습니다.", rbVscodeHint:"Remote - SSH로 같은 호스트에 연결한 뒤 원격 터미널에 프록시 변수와 CLI 명령을 붙여 넣으세요. 브리지는 ProxyEnv가 관리합니다. 터미널 export는 실행 중인 Server나 확장 호스트를 다시 설정하지 않습니다.", rbVscodeError:"VS Code에서 대상 호스트를 열 수 없습니다. VS Code와 Remote - SSH 설치 및 SSH 설정 파일 일치 여부를 확인하세요.",
  rbTitle:"원격 환경 브리지",rbHint:"SSH로 서버 또는 VM에서 현재 프록시와 CC Switch를 사용합니다.",rbOpen:"브리지 설정",rbView:"브리지 보기",rbTarget:"원격 환경 선택",rbCapabilities:"기능 선택",rbPreview:"설정 확인",rbStatus:"브리지 상태",rbAlias:"SSH 호스트 별칭",rbEmpty:"~/.ssh/config에 명시적인 Host가 없습니다. OpenSSH로 호스트를 추가한 후 새로 고치세요.",rbRequirements:"기존 키와 Agent를 사용합니다. 터미널에서 호스트 키를 먼저 확인하세요. 원격에는 ss, flock, coreutils가 있는 비 root Linux 계정이 필요합니다.",rbCheck:"연결 확인",rbChecked:"SSH 연결 확인됨",rbRefresh:"호스트 새로 고침",rbNext:"다음",rbBack:"이전",rbClose:"닫기",rbProxy:"현재 로컬 프록시",rbNoProxy:"프록시를 사용할 수 없습니다. 홈에서 사용 가능한 프록시를 선택하세요.",rbCc:"CC Switch 로컬 라우팅",rbCcHint:"포트 수신만 확인합니다. CC Switch 포트인지, 해당 CLI의 라우팅이 켜져 있는지 확인하세요.",rbLocalPort:"로컬 라우팅 포트",rbDetect:"로컬 포트 확인",rbDetected:"포트가 수신 중입니다",rbRemotePort:"원격 포트",rbPortHint:"1024–65535 범위에서 서로 다른 포트를 사용하세요.",rbSafety:"원격은 127.0.0.1에만 바인딩합니다. ProxyEnv 실행 중에만 유효하며 Shell 시작 파일을 변경하지 않습니다.",rbConnect:"브리지 연결",rbDisconnect:"브리지 연결 해제",rbDisconnectHint:"원격 프로그램의 브리지 연결이 끊깁니다. 저장된 CLI 설정은 복원할 수 있습니다.",rbConfirm:"확인",rbCancel:"취소",rbReconnect:"다시 설정",rbStaleHint:"현재 프록시가 변경되었습니다. 기존 터널은 원래 대상을 사용합니다. 연결을 해제한 후 다시 설정하세요.",rbUnavailableHint:"로컬 대상을 사용할 수 없습니다. 프록시 또는 CC Switch를 확인하세요. 대상은 자동으로 전환되지 않습니다.",rbLocal:"로컬",rbRemote:"원격",rbCopy:"환경 변수 복사",rbCopied:"복사됨",rbTest:"브리지 테스트",rbTestHint:"이 버튼만 원격 프록시를 통해 gstatic.com에 접속합니다. AI 요청은 보내지 않습니다.",rbTested:"원격 네트워크 테스트 통과",rbCodex:"Codex 설정",rbClaude:"Claude Code 설정",rbRestoreCodex:"Codex 설정 복원",rbRestoreClaude:"Claude 설정 복원",rbConfigHint:"전용 설정을 생성합니다. Codex CLI 0.134+ (0.x) 또는 Claude Code 2.x를 지원합니다. 기존 기본 설정과 인증 정보를 유지합니다. 표시된 명령으로 사용하며 상위 설정이 덮어쓸 수 있습니다. PROXY_MANAGED는 공개 자리표시자입니다.",rbBefore:"변경 전",rbAfter:"변경 후",rbAbsent:"파일 없음",rbApply:"설정 확인 후 적용",rbApplied:"설정 검증 완료",rbLaunch:"원격 실행 명령",rbCopyLaunch:"실행 명령 복사",rbRestoreHint:"원격 백업에서 원래 전용 설정을 복원합니다. 다른 프로그램이 변경했다면 덮어쓰지 않고 중단합니다.",rbRestored:"원래 설정 복원됨",rbBusy:"처리 중…",rbFailed:"작업이 실패했습니다. SSH 대상과 로컬 포트를 확인한 후 다시 시도하세요.",rbSshError:"SSH 연결 실패. 터미널에서 호스트 키, 키/Agent 인증, SSH 설정을 확인하세요. 서버 설정은 변경하지 않았습니다.",rbForwardError:"역방향 전달 실패. 원격 포트 점유와 SSH 전달 정책을 확인하세요. 서버 설정은 변경하지 않았습니다.",rbBindingError:"Loopback 전용 수신을 확인할 수 없어 새 터널을 닫았습니다. 관리자에게 GatewayPorts 정책을 확인하세요.",rbConfigError:"설정 충돌 또는 미지원 파일입니다. 충돌한 변경을 덮어쓰지 않았습니다. 원격의 전용 설정과 복구 파일을 확인하세요.",rbDependencyError:"미지원 원격 환경 또는 도구 누락입니다. ss, flock, coreutils가 있는 비 root Linux와 Codex 0.134+ (0.x) 또는 Claude Code 2.x가 필요합니다.",rbPortError:"잘못되었거나 사용 중인 포트입니다. 1024–65535 범위의 서로 다른 빈 포트를 선택하세요.",rbActiveError:"프록시가 변경되었거나 사용할 수 없습니다. 홈에서 확인한 후 다시 미리 보세요.",rbStates:{disconnected:"연결 안 됨",connecting:"연결 중",connected:"연결됨",stale:"설정 변경됨",unavailable:"사용 불가",error:"연결 실패"},
};
ja.rbRequirements = "OpenSSH の鍵と Agent を優先し、必要ならアプリ内でパスワードまたは対話認証を行います。新しいホスト鍵は先にターミナルで確認してください。接続先は非 root の Linux、ss・flock・coreutils が必要です。";
ko.rbRequirements = "OpenSSH 키와 Agent를 우선 사용하며 필요하면 앱 안에서 비밀번호 또는 대화형 인증을 진행합니다. 새 호스트 키는 터미널에서 먼저 확인하세요. 원격에는 ss, flock, coreutils가 있는 비 root Linux 계정이 필요합니다.";

export const remoteBridgeMessages = { en, "zh-CN": zh, ja, ko };
export type RemoteBridgeCopy = Labels;

export function bridgeErrorCode(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "code" in error && typeof error.code === "string") return error.code;
  return "";
}

export function bridgeError(code: unknown, copy: Labels): string {
  const value = bridgeErrorCode(code);
  if (value === "ccUnavailable") return copy.rbCcError;
  if (value === "bridgeUnavailable") return copy.rbBridgeUnavailableError;
  if (value === "noCapability") return copy.rbNoCapabilityError;
  if (value === "alreadyConnected") return copy.rbAlreadyConnectedError;
  if (value === "stateUnavailable") return copy.rbStateError;
  if (value === "processFailed") return copy.rbProcessError;
  if (value === "sshAuthRejected") return copy.rbAuthRejected;
  if (value === "sshAuthCompletionTimeout") return copy.rbAuthCompletionTimeout;
  if (value === "hostKeyChanged") return copy.rbAuthHostKeyChanged;
  if (["ptyUnavailable","sshAuthSessionMissing","sshAuthSessionClosed","sshAuthInputInvalid","sshAuthPromptChanged","sshAuthPending"].includes(value)) return copy.rbAuthError;
  if (value === "networkFailed") return copy.rbNetworkError;
  if (value === "remoteFailed") return copy.rbRemoteError;
  if (["mobaSessionUnsupported","targetUnsupported","mobaConfigInvalid"].includes(value)) return copy.rbTargetError;
  if (value === "portAllocationFailed") return copy.rbPortAllocationError;
  if (value === "portRace") return copy.rbPortRace;
  if (["extensionMissing","extensionUnsupported","extensionContextChanged","extensionLocationRequired"].includes(value)) return copy.rbExtError;
  if (["vscodeMissing","vscodeConfigInvalid","vscodeConfigMismatch"].includes(value)) return copy.rbVscodeError;
  if (["sshConfigChanged","sshMissing","sshConfigMissing","sshConfigUnsafe","sshTimeout","hostKey","sshAuth","sshFailed","invalidTarget"].includes(value)) return copy.rbSshError;
  if (value === "forwardDenied") return copy.rbForwardError;
  if (value === "unsafeBinding") return copy.rbBindingError;
  if (["configConflict","unsafePath","noBackup","rollbackConflict","rollbackFailed","writeRolledBack","verifyFailed"].includes(value)) return copy.rbConfigError;
  if (["remoteUnsupported","dependencyMissing","cliUnsupported","customHome","rootForbidden"].includes(value)) return copy.rbDependencyError;
  if (["portInUse","invalidPort"].includes(value)) return copy.rbPortError;
  if (["proxyUnavailable","activeChanged"].includes(value)) return copy.rbActiveError;
  return copy.rbFailed;
}
