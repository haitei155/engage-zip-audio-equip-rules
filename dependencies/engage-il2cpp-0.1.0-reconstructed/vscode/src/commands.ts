import * as vscode from "vscode";

import { resolveBinary } from "./binary";
import { runEngage, workspaceCwd } from "./runner";

export function registerCommands(context: vscode.ExtensionContext, output: vscode.OutputChannel): void {
    context.subscriptions.push(
        vscode.commands.registerCommand("engage.featuresCheck", () => featuresCheck(context, output)),
        vscode.commands.registerCommand("engage.featuresApply", () => featuresApply(context, output)),
        vscode.commands.registerCommand("engage.featuresPrune", () => featuresPrune(context, output)),
        vscode.commands.registerCommand("engage.featuresExplain", () => featuresExplain(context, output)),
        vscode.commands.registerCommand("engage.addFeature", (name: string) => addFeature(context, output, name)),
    );
}

async function featuresCheck(context: vscode.ExtensionContext, output: vscode.OutputChannel) {
    await runVerb(context, output, ["features", "check"], "Checking features");
}

async function featuresApply(context: vscode.ExtensionContext, output: vscode.OutputChannel) {
    await runVerb(context, output, ["features", "apply", "--yes"], "Applying missing features");
}

async function featuresPrune(context: vscode.ExtensionContext, output: vscode.OutputChannel) {
    const choice = await vscode.window.showWarningMessage(
        "Remove every engage-il2cpp feature that nothing references?",
        { modal: true },
        "Prune",
    );

    if (choice !== "Prune") {
        return;
    }

    await runVerb(context, output, ["features", "prune", "--yes"], "Pruning unused features");
}

async function featuresExplain(context: vscode.ExtensionContext, output: vscode.OutputChannel) {
    const path = await vscode.window.showInputBox({
        prompt: "Enter a fully-qualified Rust path",
        placeHolder: "engage::app::unit::Unit",
    });

    if (!path) {
        return;
    }

    await runVerb(context, output, ["features", "explain", path], `Explaining ${path}`);
}

async function addFeature(context: vscode.ExtensionContext, output: vscode.OutputChannel, name: string): Promise<void> {
    if (!name) {
        return;
    }

    await runVerb(context, output, ["features", "add", name], `Adding feature ${name}`);
}

async function runVerb(context: vscode.ExtensionContext, output: vscode.OutputChannel, args: string[], title: string): Promise<void> {
    const binary = resolveBinary(context);
    const cwd = workspaceCwd();

    output.show(true);
    output.appendLine(`> ${title}`);
    output.appendLine(`$ ${binary} ${args.join(" ")}`);

    try {
        const result = await runEngage(binary, args, cwd);

        if (result.stdout) {
            output.append(result.stdout);
        }

        if (result.stderr) {
            output.append(result.stderr);
        }

        output.appendLine(`(exited ${result.code ?? "?"})`);

        if (result.stderr.includes("no workspace member depends on `engage-il2cpp`")) {
            vscode.window.showInformationMessage("engage: this workspace does not depend on engage-il2cpp.");
        }
    } catch (err) {
        const message = err instanceof Error ? err.message : String(err);

        output.appendLine(`failed to spawn ${binary}: ${message}`);
        vscode.window.showErrorMessage(`engage: failed to spawn binary. ${message}`);
    }
}
