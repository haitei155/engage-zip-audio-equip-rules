import * as vscode from "vscode";

import { registerCodeActions } from "./codeActions";
import { registerCommands } from "./commands";
import { registerCompletions } from "./completions";
import { activateDiagnostics } from "./diagnostics";
import { activateLsp, deactivateLsp } from "./lsp";
import { activateUpdater } from "./updater";

let outputChannel: vscode.OutputChannel | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
    outputChannel = vscode.window.createOutputChannel("engage");
    context.subscriptions.push(outputChannel);

    registerCommands(context, outputChannel);
    registerCodeActions(context);
    registerCompletions(context, outputChannel);
    activateDiagnostics(context, outputChannel);
    activateUpdater(context, outputChannel);

    await activateLsp(context, outputChannel);
}

export async function deactivate(): Promise<void> {
    await deactivateLsp();

    if (outputChannel) {
        outputChannel.dispose();
        outputChannel = undefined;
    }
}
