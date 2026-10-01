import * as path from "path";
import * as fs from "fs";
import * as vscode from "vscode";

export function resolveBinary(context: vscode.ExtensionContext): string {
    const override = vscode.workspace.getConfiguration("engage").get<string>("binaryPath");

    if (override && override.trim().length > 0) {
        return override;
    }

    const exeName = process.platform === "win32" ? "engage.exe" : "engage";
    const bundled = path.join(context.extensionPath, "bin", exeName);

    if (fs.existsSync(bundled)) {
        return bundled;
    }

    return "engage";
}
