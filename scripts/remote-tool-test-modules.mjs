import {readFileSync} from "node:fs";
import ts from "typescript";

const dataUrl=source => `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`;
const compile=path => ts.transpileModule(readFileSync(path,"utf8"),{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
export const adapterModuleUrl=dataUrl(compile("src/features/remote-bridge/tool-adapters.ts").replace(/import \{ remoteBackend \} from "\.\/state";/,'const remoteBackend={};'));
export const presentationModuleUrl=dataUrl(compile("src/features/remote-bridge/overview-presentation.ts").replace('"./tool-adapters"',JSON.stringify(adapterModuleUrl)));
export const guidanceModuleUrl=dataUrl(compile("src/features/remote-bridge/install-guidance.ts"));
export const copyModuleUrl=dataUrl(compile("src/shared/i18n/remote-bridge.ts"));
