"use strict";
// Volt HDL VS Code istemcisi — `volt lsp` sunucusuna stdio ile bağlanır.
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = require("vscode");
const node_1 = require("vscode-languageclient/node");
let client;
function activate(context) {
    const serverPath = vscode.workspace
        .getConfiguration("volt")
        .get("serverPath", "volt");
    const serverOptions = {
        command: serverPath,
        args: ["lsp"],
    };
    const clientOptions = {
        documentSelector: [{ scheme: "file", language: "volt" }],
    };
    client = new node_1.LanguageClient("volt-lsp", "Volt HDL Language Server", serverOptions, clientOptions);
    client.start().catch((err) => {
        vscode.window.showErrorMessage(`Volt HDL: language server could not start (${serverPath} lsp): ${err}`);
    });
    context.subscriptions.push({
        dispose: () => {
            void client?.stop();
        },
    });
}
function deactivate() {
    return client?.stop();
}
//# sourceMappingURL=extension.js.map