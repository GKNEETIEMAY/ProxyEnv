// Report labels live in the shared i18n catalog; state meaning stays in the domain enums.
const en = {
  reportTitle: "ProxyEnv Diagnostic Report", reportOpen: "Diagnostic report", reportLanguage: "Report language",
  reportFollowInterface: "Follow interface language", reportCopy: "Copy diagnostic report", reportRefresh: "Refresh report",
  reportPrivacy: "Only safe status summaries are included. Paths, credentials, addresses and configuration values are excluded. Nothing is saved or uploaded automatically.",
  reportSnapshot: "Read-only snapshot. Connectivity uses the latest valid test, not a new network request. Application details describe only the selected app.",
  reportLoading: "Collecting safe diagnostic data…", reportLoadFailed: "Could not generate the report. Refresh and try again.",
  reportCopied: "Diagnostic report copied", reportCopyFailed: "Copy failed. Select the preview text and copy it manually.",
  reportPreview: "Diagnostic report preview", reportVersion: "ProxyEnv version", reportOs: "Operating system version",
  reportDetected: "Detected clients", reportListening: "Listening clients", reportSelected: "Selected client",
  reportStatus: "Status", reportManagedVariables: "Managed variable names", reportConnectivity: "Connectivity",
  reportSuccessfulTargets: "Successful targets", reportErrors: "Error categories", reportCategory: "Application category",
  reportNone: "None", reportUnknown: "Unknown", reportNotSelected: "Not selected", reportKnownRule: "Application with a known rule",
  reportUnrecognized: "Unrecognized application", reportOtherClient: "Other / manual proxy",
  reportNotTested: "Not tested", reportTesting: "Testing", reportReachable: "Reachable", reportPartial: "Partially reachable", reportUnreachable: "Unreachable",
  reportProxyUnavailable: "Local proxy unavailable", reportHandshakeFailed: "Proxy handshake failed", reportTimeout: "Connection timed out",
  reportTlsFailed: "TLS failed", reportRemoteRejected: "Remote request rejected", reportHttpStatus: "Unexpected HTTP status", reportNetworkError: "Network error",
  reportVeryHigh: "Very high", reportHigh: "High", reportMedium: "Medium", reportLow: "Low"
};

type Labels = { [K in keyof typeof en]: string };
const zh: Labels = {
  reportTitle: "ProxyEnv 诊断报告", reportOpen: "诊断报告", reportLanguage: "报告语言",
  reportFollowInterface: "跟随界面语言", reportCopy: "复制诊断报告", reportRefresh: "刷新报告",
  reportPrivacy: "仅包含安全的状态摘要，不包含路径、凭据、地址或配置原值。不会自动保存或上传。",
  reportSnapshot: "只读状态快照。连通性引用最近一次有效测试，不会发起新请求；应用部分仅描述当前选中的应用。",
  reportLoading: "正在收集安全诊断数据…", reportLoadFailed: "无法生成诊断报告，请刷新后重试。",
  reportCopied: "诊断报告已复制", reportCopyFailed: "复制失败，请选中预览文本后手动复制。",
  reportPreview: "诊断报告预览", reportVersion: "ProxyEnv 版本", reportOs: "操作系统版本",
  reportDetected: "检测到的客户端", reportListening: "正在监听的客户端", reportSelected: "当前选择",
  reportStatus: "状态", reportManagedVariables: "管理变量名称", reportConnectivity: "连通性测试",
  reportSuccessfulTargets: "成功目标", reportErrors: "错误类别", reportCategory: "应用类别",
  reportNone: "无", reportUnknown: "未知", reportNotSelected: "未选择", reportKnownRule: "有已知规则的应用",
  reportUnrecognized: "未识别的应用", reportOtherClient: "其他 / 手动代理",
  reportNotTested: "未测试", reportTesting: "测试中", reportReachable: "可连接", reportPartial: "部分可连接", reportUnreachable: "无法连接",
  reportProxyUnavailable: "本地代理不可用", reportHandshakeFailed: "代理握手失败", reportTimeout: "连接超时",
  reportTlsFailed: "TLS 失败", reportRemoteRejected: "远程请求被拒绝", reportHttpStatus: "HTTP 状态异常", reportNetworkError: "网络错误",
  reportVeryHigh: "很高", reportHigh: "高", reportMedium: "中", reportLow: "低"
};
const ja: Labels = {
  reportTitle: "ProxyEnv 診断レポート", reportOpen: "診断レポート", reportLanguage: "レポートの言語",
  reportFollowInterface: "表示言語に合わせる", reportCopy: "診断レポートをコピー", reportRefresh: "レポートを更新",
  reportPrivacy: "安全な状態の概要のみを含みます。パス、認証情報、アドレス、設定の元の値は含まれません。自動保存・送信は行いません。",
  reportSnapshot: "読み取り専用の状態記録です。接続状態には直近の有効なテストを使用し、新たな通信は行いません。アプリの情報は現在の選択のみを対象とします。",
  reportLoading: "安全な診断データを収集中…", reportLoadFailed: "レポートを生成できませんでした。更新して再試行してください。",
  reportCopied: "診断レポートをコピーしました", reportCopyFailed: "コピーできませんでした。プレビューのテキストを選択して手動でコピーしてください。",
  reportPreview: "診断レポートのプレビュー", reportVersion: "ProxyEnv バージョン", reportOs: "OS バージョン",
  reportDetected: "検出したクライアント数", reportListening: "待受中のクライアント数", reportSelected: "選択中のクライアント",
  reportStatus: "状態", reportManagedVariables: "管理対象の変数名", reportConnectivity: "接続テスト",
  reportSuccessfulTargets: "成功した接続先", reportErrors: "エラー分類", reportCategory: "アプリの分類",
  reportNone: "なし", reportUnknown: "不明", reportNotSelected: "未選択", reportKnownRule: "既知のルールがあるアプリ",
  reportUnrecognized: "未認識のアプリ", reportOtherClient: "その他 / 手動プロキシ",
  reportNotTested: "未テスト", reportTesting: "テスト中", reportReachable: "接続可能", reportPartial: "一部接続可能", reportUnreachable: "接続不可",
  reportProxyUnavailable: "ローカルプロキシが利用不可", reportHandshakeFailed: "プロキシのハンドシェイク失敗", reportTimeout: "接続タイムアウト",
  reportTlsFailed: "TLS 失敗", reportRemoteRejected: "リモート要求が拒否されました", reportHttpStatus: "予期しない HTTP 状態", reportNetworkError: "ネットワークエラー",
  reportVeryHigh: "非常に高い", reportHigh: "高い", reportMedium: "中程度", reportLow: "低い"
};
const ko: Labels = {
  reportTitle: "ProxyEnv 진단 보고서", reportOpen: "진단 보고서", reportLanguage: "보고서 언어",
  reportFollowInterface: "화면 언어 따르기", reportCopy: "진단 보고서 복사", reportRefresh: "보고서 새로 고침",
  reportPrivacy: "안전한 상태 요약만 포함합니다. 경로, 인증 정보, 주소 및 설정 원본 값은 제외됩니다. 자동으로 저장하거나 업로드하지 않습니다.",
  reportSnapshot: "읽기 전용 상태 기록입니다. 연결 상태는 최근 유효한 테스트를 사용하며 새 요청을 보내지 않습니다. 앱 정보는 현재 선택된 앱만 나타냅니다.",
  reportLoading: "안전한 진단 데이터 수집 중…", reportLoadFailed: "보고서를 생성할 수 없습니다. 새로 고침 후 다시 시도하세요.",
  reportCopied: "진단 보고서를 복사했습니다", reportCopyFailed: "복사하지 못했습니다. 미리보기 텍스트를 선택하여 직접 복사하세요.",
  reportPreview: "진단 보고서 미리보기", reportVersion: "ProxyEnv 버전", reportOs: "운영 체제 버전",
  reportDetected: "감지된 클라이언트 수", reportListening: "수신 중인 클라이언트 수", reportSelected: "선택된 클라이언트",
  reportStatus: "상태", reportManagedVariables: "관리 변수 이름", reportConnectivity: "연결 테스트",
  reportSuccessfulTargets: "성공한 대상", reportErrors: "오류 분류", reportCategory: "앱 분류",
  reportNone: "없음", reportUnknown: "알 수 없음", reportNotSelected: "선택 안 됨", reportKnownRule: "알려진 규칙이 있는 앱",
  reportUnrecognized: "인식되지 않은 앱", reportOtherClient: "기타 / 수동 프록시",
  reportNotTested: "테스트하지 않음", reportTesting: "테스트 중", reportReachable: "연결 가능", reportPartial: "일부 연결 가능", reportUnreachable: "연결 불가",
  reportProxyUnavailable: "로컬 프록시 사용 불가", reportHandshakeFailed: "프록시 핸드셰이크 실패", reportTimeout: "연결 시간 초과",
  reportTlsFailed: "TLS 실패", reportRemoteRejected: "원격 요청 거부", reportHttpStatus: "예상하지 못한 HTTP 상태", reportNetworkError: "네트워크 오류",
  reportVeryHigh: "매우 높음", reportHigh: "높음", reportMedium: "보통", reportLow: "낮음"
};

export const diagnosticReportMessages = { en, "zh-CN": zh, ja, ko };
