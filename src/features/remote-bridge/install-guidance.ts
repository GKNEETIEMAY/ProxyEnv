import type { RemoteToolId } from "./tool-adapters";
import type { RemoteInstallCapabilities } from "./state";

export interface ToolInstallMethod {
  id:"native"|"npm"|"homebrew";
  label:string;
  command:string;
  verifyCommand:string;
  officialUrl:string;
  requirement:"curl"|"npm"|"brew";
  recommended:boolean;
  note?:"noSudo";
}
// Reviewed static commands only. Remote discovery can never provide executable text.
const catalog:Record<RemoteToolId,readonly ToolInstallMethod[]> = {
  codex:[
    {id:"native",label:"Native Installer",command:"curl -fsSL https://chatgpt.com/codex/install.sh | sh",verifyCommand:"codex --version",officialUrl:"https://github.com/openai/codex/blob/main/README.md",requirement:"curl",recommended:true},
    {id:"npm",label:"npm",command:"npm install -g @openai/codex",verifyCommand:"codex --version",officialUrl:"https://github.com/openai/codex/blob/main/README.md",requirement:"npm",recommended:false,note:"noSudo"},
    {id:"homebrew",label:"Homebrew",command:"brew install --cask codex",verifyCommand:"codex --version",officialUrl:"https://github.com/openai/codex/blob/main/README.md",requirement:"brew",recommended:false},
  ],
  claude:[
    {id:"native",label:"Native Installer",command:"curl -fsSL https://claude.ai/install.sh | bash",verifyCommand:"claude --version",officialUrl:"https://code.claude.com/docs/en/setup",requirement:"curl",recommended:true},
    {id:"npm",label:"npm",command:"npm install -g @anthropic-ai/claude-code",verifyCommand:"claude --version",officialUrl:"https://code.claude.com/docs/en/setup",requirement:"npm",recommended:false,note:"noSudo"},
    {id:"homebrew",label:"Homebrew",command:"brew install --cask claude-code",verifyCommand:"claude --version",officialUrl:"https://code.claude.com/docs/en/setup",requirement:"brew",recommended:false},
  ],
};
export const installMethods = (tool:RemoteToolId) => catalog[tool];
export function installMissingRequirements(tool:RemoteToolId, method:ToolInstallMethod, capabilities?:RemoteInstallCapabilities):Array<ToolInstallMethod["requirement"]|"node"> {
  const missing:Array<ToolInstallMethod["requirement"]|"node">=[];
  if (!capabilities?.[method.requirement]) missing.push(method.requirement);
  // Project baseline remains 18+; current official guidance is shown separately.
  if (tool === "claude" && method.id === "npm" && Number(capabilities?.nodeVersion?.split(".")[0] ?? 0) < 18) missing.push("node");
  return missing;
}
export function installRequirementAvailable(tool:RemoteToolId, method:ToolInstallMethod, capabilities?:RemoteInstallCapabilities):boolean {
  return installMissingRequirements(tool,method,capabilities).length === 0;
}
