import * as path from "path";
import * as fs from "fs";
import * as vscode from "vscode";

import { resolveBinary } from "./binary";
import { runEngage, workspaceCwd } from "./runner";

export interface CheckResult {
    missing: string[];
    stale: string[];
}

export function activateDiagnostics(context: vscode.ExtensionContext, output: vscode.OutputChannel): vscode.DiagnosticCollection {
    const collection = vscode.languages.createDiagnosticCollection("engage");
    context.subscriptions.push(collection);

    const refresh = (doc: vscode.TextDocument) => {
        const config = vscode.workspace.getConfiguration("engage");

        if (!isWatchedDocument(doc)) {
            return;
        }

        const checkOnSave = config.get<boolean>("checkOnSave", true);
        const autoApply = config.get<boolean>("autoApplyOnSave", false);
        const autoPrune = config.get<boolean>("autoPruneOnSave", false);

        if (!checkOnSave && !autoApply && !autoPrune) {
            return;
        }

        runDiagnostics(context, output, collection, autoApply, autoPrune, checkOnSave).catch((err) => {
            output.appendLine(`diagnostics refresh failed: ${err}`);
        });
    };

    context.subscriptions.push(
        vscode.workspace.onDidSaveTextDocument(refresh),
    );

    return collection;
}

export async function runDiagnostics(
    context: vscode.ExtensionContext,
    output: vscode.OutputChannel,
    collection: vscode.DiagnosticCollection,
    autoApply: boolean = false,
    autoPrune: boolean = false,
    publishDiagnostics: boolean = true,
): Promise<void> {
    const binary = resolveBinary(context);
    const cwd = workspaceCwd();

    if (!cwd) {
        return;
    }

    const parsed = await runCheck(binary, cwd, output);

    if (!parsed) {
        return;
    }

    let didMutate = false;

    if (autoApply && parsed.missing.length > 0) {
        const result = await runEngage(binary, ["features", "apply", "--yes"], cwd).catch(() => undefined);

        if (result) {
            output.appendLine(`auto-applied ${parsed.missing.length} feature${parsed.missing.length === 1 ? "" : "s"}`);
            didMutate = true;
        }
    }

    if (autoPrune && parsed.stale.length > 0) {
        const result = await runEngage(binary, ["features", "prune", "--yes"], cwd).catch(() => undefined);

        if (result) {
            output.appendLine(`auto-pruned ${parsed.stale.length} feature${parsed.stale.length === 1 ? "" : "s"}`);
            didMutate = true;
        }
    }

    if (!publishDiagnostics) {
        return;
    }

    const finalParsed = didMutate ? await runCheck(binary, cwd, output) : parsed;

    if (!finalParsed) {
        return;
    }

    const cargoToml = findCargoToml(cwd);

    if (!cargoToml) {
        return;
    }

    if (finalParsed.missing.length === 0) {
        collection.set(vscode.Uri.file(cargoToml), []);
        return;
    }

    const range = locateEngageIl2cppLine(cargoToml);
    const message = `engage-il2cpp: missing ${finalParsed.missing.length} required feature${finalParsed.missing.length === 1 ? "" : "s"}: ${finalParsed.missing.join(", ")}`;
    const diag = new vscode.Diagnostic(range, message, vscode.DiagnosticSeverity.Warning);
    diag.source = "engage";
    diag.code = "missing-features";

    collection.set(vscode.Uri.file(cargoToml), [diag]);
}

async function runCheck(binary: string, cwd: string, output: vscode.OutputChannel): Promise<CheckResult | undefined> {
    let result;

    try {
        result = await runEngage(binary, ["features", "check"], cwd);
    } catch (err) {
        output.appendLine(`failed to run engage features check: ${err}`);
        return undefined;
    }

    if (isNotEngageProject(result.stderr)) {
        return undefined;
    }

    return parseCheckOutput(result.stdout);
}

function parseCheckOutput(stdout: string): CheckResult {
    const lines = stdout.split(/\r?\n/);
    const missing: string[] = [];
    const stale: string[] = [];
    let mode: "none" | "missing" | "stale" = "none";

    for (const line of lines) {
        if (line.startsWith("missing (")) {
            mode = "missing";
            continue;
        }

        if (line.startsWith("stale (")) {
            mode = "stale";
            continue;
        }

        if (line.startsWith("  + ") && mode === "missing") {
            missing.push(line.slice(4).trim());
            continue;
        }

        if (line.startsWith("  - ") && mode === "stale") {
            stale.push(line.slice(4).trim());
            continue;
        }

        if (line.length === 0) {
            mode = "none";
        }
    }

    return { missing, stale };
}

function isWatchedDocument(doc: vscode.TextDocument): boolean {
    if (doc.languageId === "rust") {
        return true;
    }

    if (path.basename(doc.fileName) === "Cargo.toml") {
        return true;
    }

    return false;
}

function findCargoToml(cwd: string): string | undefined {
    const candidate = path.join(cwd, "Cargo.toml");

    if (fs.existsSync(candidate)) {
        return candidate;
    }

    return undefined;
}

function isNotEngageProject(stderr: string): boolean {
    return stderr.includes("no workspace member depends on `engage-il2cpp`");
}

function locateEngageIl2cppLine(cargoToml: string): vscode.Range {
    let raw: string;

    try {
        raw = fs.readFileSync(cargoToml, "utf8");
    } catch {
        return new vscode.Range(0, 0, 0, 0);
    }

    const lines = raw.split(/\r?\n/);

    for (let i = 0; i < lines.length; i++) {
        if (lines[i].includes("engage-il2cpp")) {
            return new vscode.Range(i, 0, i, lines[i].length);
        }
    }

    return new vscode.Range(0, 0, 0, 0);
}
