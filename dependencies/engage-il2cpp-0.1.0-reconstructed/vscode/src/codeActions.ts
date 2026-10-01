import * as vscode from "vscode";

import { resolveBinary } from "./binary";
import { runEngage, workspaceCwd } from "./runner";

export class EngageCodeActionProvider implements vscode.CodeActionProvider {
    public static readonly providedKinds = [vscode.CodeActionKind.QuickFix];

    provideCodeActions(
        _document: vscode.TextDocument,
        _range: vscode.Range,
        context: vscode.CodeActionContext,
    ): vscode.CodeAction[] {
        const actions: vscode.CodeAction[] = [];

        for (const diag of context.diagnostics) {
            if (diag.source !== "engage") {
                continue;
            }

            if (diag.code === "missing-features") {
                const fix = new vscode.CodeAction("engage: Apply missing features", vscode.CodeActionKind.QuickFix);
                fix.command = { command: "engage.featuresApply", title: "Apply missing features" };
                fix.diagnostics = [diag];
                fix.isPreferred = true;
                actions.push(fix);
            }
        }

        return actions;
    }
}

/// Catches rustc `E0599 no method named X found for struct Y` diagnostics on
/// Rust files and offers an "Enable feature gating Y's methods" quick-fix.
///
/// The receiver's methods feature gates the I<Y>Methods trait, which in turn
/// transitively pulls in parent methods features via the bindgen-emitted
/// feature cascade. So enabling the receiver's methods feature is enough to
/// resolve the call even when the method actually lives on an ancestor trait.
export class EngageRustcMethodFixProvider implements vscode.CodeActionProvider {
    public static readonly providedKinds = [vscode.CodeActionKind.QuickFix];

    private readonly cache = new Map<string, Promise<string[]>>();
    private readonly context: vscode.ExtensionContext;

    constructor(context: vscode.ExtensionContext) {
        this.context = context;
    }

    async provideCodeActions(
        _document: vscode.TextDocument,
        _range: vscode.Range,
        context: vscode.CodeActionContext,
    ): Promise<vscode.CodeAction[]> {
        const actions: vscode.CodeAction[] = [];

        for (const diag of context.diagnostics) {
            const source = diag.source ?? "";

            if (source !== "rustc" && source !== "rust-analyzer") {
                continue;
            }

            const receiver = parseNoMethodReceiver(diag.message);

            if (!receiver) {
                continue;
            }

            const features = await this.lookup(receiver);

            for (const feature of features) {
                const title = `engage: Enable feature \`${feature}\` for \`${receiver.split("::").pop()}\``;
                const fix = new vscode.CodeAction(title, vscode.CodeActionKind.QuickFix);
                fix.command = {
                    command: "engage.addFeature",
                    title: "Add feature",
                    arguments: [feature],
                };
                fix.diagnostics = [diag];
                fix.isPreferred = features.length === 1;
                actions.push(fix);
            }
        }

        return actions;
    }

    private lookup(receiver: string): Promise<string[]> {
        const cached = this.cache.get(receiver);

        if (cached) {
            return cached;
        }

        const promise = this.runLookup(receiver);
        this.cache.set(receiver, promise);
        return promise;
    }

    private async runLookup(receiver: string): Promise<string[]> {
        const binary = resolveBinary(this.context);
        const cwd = workspaceCwd();

        if (!cwd) {
            return [];
        }

        try {
            const result = await runEngage(binary, ["features", "for-receiver", receiver], cwd);

            return result.stdout
                .split(/\r?\n/)
                .map((line) => line.trim())
                .filter((line) => line.length > 0);
        } catch {
            return [];
        }
    }
}

/// Extracts the receiver path from a rustc `no method named X found for struct Y`
/// message. Returns the canonical engage-il2cpp-style path with `generated::`
/// and `__types::` stripped, or `undefined` if the message shape doesn't match.
function parseNoMethodReceiver(message: string): string | undefined {
    // E0599 ships under at least two phrasings:
    //   1. `no method named X found for struct Y in the current scope`
    //   2. `the method X exists for struct Y, but its trait bounds were not satisfied`
    // Case 2 is what fires when the *types* feature for Y is on (so Y is a
    // real struct) but the *methods* feature is off (so the I<Y>Methods
    // trait's bound `Self: I<Y>` isn't satisfied for the receiver). Both
    // resolve to the same fix: enable Y's methods feature.
    const patterns = [
        /no method named `[^`]+` found for (?:struct|enum|type|reference|associated type) `([^`]+)`/,
        /the method `[^`]+` exists for (?:struct|enum|type|reference|associated type) `([^`]+)`, but its trait bounds were not satisfied/,
    ];

    let captured: string | undefined;

    for (const re of patterns) {
        const m = message.match(re);

        if (m) {
            captured = m[1];
            break;
        }
    }

    if (!captured) {
        return undefined;
    }

    return captured
        .replace(/&(?:mut\s+)?/g, "")
        .replace(/r#/g, "")
        .replace(/::generated::/g, "::")
        .replace(/::__types::/g, "::")
        .trim();
}

export function registerCodeActions(context: vscode.ExtensionContext): void {
    context.subscriptions.push(
        vscode.languages.registerCodeActionsProvider(
            { scheme: "file", pattern: "**/Cargo.toml" },
            new EngageCodeActionProvider(),
            { providedCodeActionKinds: EngageCodeActionProvider.providedKinds },
        ),
        vscode.languages.registerCodeActionsProvider(
            { scheme: "file", language: "rust" },
            new EngageRustcMethodFixProvider(context),
            { providedCodeActionKinds: EngageRustcMethodFixProvider.providedKinds },
        ),
    );
}
