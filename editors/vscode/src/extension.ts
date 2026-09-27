// Volt HDL VS Code istemcisi — `volt lsp` sunucusuna stdio ile bağlanır.

import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
  const serverPath = vscode.workspace
    .getConfiguration("volt")
    .get<string>("serverPath", "volt");

  const serverOptions: ServerOptions = {
    command: serverPath,
    args: ["lsp"],
  };

  // Inlay ipucu türleri (ADR-0091): başlangıçta initializationOptions,
  // değişince workspace/didChangeConfiguration ({ volt: { inlayHints } }).
  const inlayHints = vscode.workspace.getConfiguration("volt.inlayHints");
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "volt" }],
    initializationOptions: {
      inlayHints: {
        types: inlayHints.get<boolean>("types", true),
        clockDomains: inlayHints.get<boolean>("clockDomains", true),
        latency: inlayHints.get<boolean>("latency", true),
      },
    },
    synchronize: { configurationSection: "volt" },
  };

  client = new LanguageClient(
    "volt-lsp",
    "Volt HDL Language Server",
    serverOptions,
    clientOptions
  );

  client.start().catch((err) => {
    vscode.window.showErrorMessage(
      `Volt HDL: language server could not start (${serverPath} lsp): ${err}`
    );
  });

  context.subscriptions.push({
    dispose: () => {
      void client?.stop();
    },
  });
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
