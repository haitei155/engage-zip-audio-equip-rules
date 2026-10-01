import * as path from "path";
import * as fs from "fs";
import * as vscode from "vscode";
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind } from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export async function activateLsp(context: vscode.ExtensionContext, output: vscode.OutputChannel): Promise<void> {
    const enabled = vscode.workspace.getConfiguration("engage").get<boolean>("lspEnabled", true);

    if (!enabled) {
        return;
    }

    const binary = resolveLspBinary(context);

    if (!binary) {
        output.appendLine("engage-lsp binary not found, skipping language server");
        return;
    }

    const serverOptions: ServerOptions = {
        run: { command: binary, transport: TransportKind.stdio },
        debug: { command: binary, transport: TransportKind.stdio },
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [
            { scheme: "file", language: "rust" },
            { scheme: "file", pattern: "**/Cargo.toml" },
        ],
        synchronize: {
            fileEvents: [
                vscode.workspace.createFileSystemWatcher("**/Cargo.toml"),
                vscode.workspace.createFileSystemWatcher("**/*.rs"),
            ],
        },
        outputChannel: output,
    };

    client = new LanguageClient("engage-lsp", "engage", serverOptions, clientOptions);

    try {
        await client.start();
    } catch (err) {
        const message = err instanceof Error ? err.message : String(err);

        output.appendLine(`failed to start engage-lsp: ${message}`);
        client = undefined;
    }
}

export async function deactivateLsp(): Promise<void> {
    if (!client) {
        return;
    }

    await client.stop();
    client = undefined;
}

function resolveLspBinary(context: vscode.ExtensionContext): string | undefined {
    const exeName = process.platform === "win32" ? "engage-lsp.exe" : "engage-lsp";
    const bundled = path.join(context.extensionPath, "bin", exeName);

    if (fs.existsSync(bundled)) {
        return bundled;
    }

    return undefined;
}
