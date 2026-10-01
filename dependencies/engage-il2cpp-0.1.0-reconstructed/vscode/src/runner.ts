import { spawn } from "child_process";
import * as vscode from "vscode";

export interface RunResult {
    code: number | null;
    stdout: string;
    stderr: string;
}

export function runEngage(binary: string, args: string[], cwd: string | undefined): Promise<RunResult> {
    return new Promise((resolve, reject) => {
        const proc = spawn(binary, args, {
            cwd,
            env: process.env,
            shell: false,
        });

        let stdout = "";
        let stderr = "";

        proc.stdout.on("data", (chunk) => {
            stdout += chunk.toString();
        });

        proc.stderr.on("data", (chunk) => {
            stderr += chunk.toString();
        });

        proc.on("error", (err) => {
            reject(err);
        });

        proc.on("close", (code) => {
            resolve({ code, stdout, stderr });
        });
    });
}

export function workspaceCwd(): string | undefined {
    const folders = vscode.workspace.workspaceFolders;

    if (!folders || folders.length === 0) {
        return undefined;
    }

    return folders[0].uri.fsPath;
}
