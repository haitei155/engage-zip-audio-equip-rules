import * as path from "path";
import * as vscode from "vscode";

import { resolveBinary } from "./binary";
import { runEngage, workspaceCwd } from "./runner";

interface DisabledEntry {
    path: string;
    feature: string;
    leaf: string;
}

/// Suggests engage-il2cpp types whose gating Cargo feature is currently
/// disabled. rust-analyzer can't see those (the gated module isn't
/// compiled into the workspace), so without this provider you'd have to
/// know the type name and the feature off the top of your head.
///
/// On accept: inserts `use <full_path>;` after the last existing `use`
/// line and queues `engage.addFeature <feature>` so Cargo.toml is
/// updated before the next build.
///
/// Only fires when the leaf name typed by the user is at least
/// MIN_PREFIX_LEN characters long, so the menu doesn't drown in 8.5k
/// entries on every keystroke.
export class EngageCompletionProvider implements vscode.CompletionItemProvider {
    private static readonly MIN_PREFIX_LEN = 1;

    private readonly context: vscode.ExtensionContext;
    private readonly output: vscode.OutputChannel;

    private entries: DisabledEntry[] = [];
    private loadPromise: Promise<void> | undefined;

    constructor(context: vscode.ExtensionContext, output: vscode.OutputChannel) {
        this.context = context;
        this.output = output;
    }

    async provideCompletionItems(
        document: vscode.TextDocument,
        position: vscode.Position,
        _token: vscode.CancellationToken,
        context: vscode.CompletionContext,
    ): Promise<vscode.CompletionList<vscode.CompletionItem>> {
        if (suppressedAtPosition(document, position)) {
            return new vscode.CompletionList([], false);
        }

        await this.ensureLoaded();

        if (this.entries.length === 0) {
            return new vscode.CompletionList([], false);
        }

        const typed = wordAtPosition(document, position);

        if (
            typed.length < EngageCompletionProvider.MIN_PREFIX_LEN &&
            context.triggerKind !== vscode.CompletionTriggerKind.Invoke
        ) {
            return new vscode.CompletionList([], false);
        }

        const typedLower = typed.toLowerCase();
        const insertLine = findUseInsertLine(document);
        const insertPos = new vscode.Position(insertLine, 0);

        const items: vscode.CompletionItem[] = [];

        for (const entry of this.entries) {
            const leafLower = entry.leaf.toLowerCase();

            if (typedLower.length > 0 && !leafLower.startsWith(typedLower)) {
                continue;
            }

            const item = new vscode.CompletionItem(entry.leaf, vscode.CompletionItemKind.Struct);
            item.detail = `engage · ${entry.feature}`;
            item.documentation = new vscode.MarkdownString(
                `\`${entry.path}\`\n\nAccepting this completion enables feature \`${entry.feature}\` in \`Cargo.toml\`.`,
            );
            item.additionalTextEdits = [
                vscode.TextEdit.insert(insertPos, `use ${entry.path};\n`),
            ];
            item.command = {
                command: "engage.addFeature",
                title: "Add engage feature",
                arguments: [entry.feature],
            };
            // Sort engage suggestions below rust-analyzer's native ones for
            // any matching label — RA's are usually the right pick when both
            // exist (e.g. when the feature is currently enabled).
            item.sortText = `~engage~${entry.leaf}`;
            items.push(item);

            if (items.length >= 200) {
                break;
            }
        }

        // `isIncomplete: true` makes VS Code re-invoke the provider on every
        // keystroke instead of filtering a single cached batch client-side.
        // Without it, an early `[]` (e.g. when only one char was typed) gets
        // cached and the menu stays empty until something invalidates it.
        return new vscode.CompletionList(items, true);
    }

    refresh(): void {
        this.entries = [];
        this.loadPromise = undefined;
    }

    private ensureLoaded(): Promise<void> {
        if (this.loadPromise) {
            return this.loadPromise;
        }

        this.loadPromise = this.load();
        return this.loadPromise;
    }

    private async load(): Promise<void> {
        const binary = resolveBinary(this.context);
        const cwd = workspaceCwd();

        if (!cwd) {
            return;
        }

        try {
            const result = await runEngage(binary, ["features", "list-disabled", "--json"], cwd);

            if (result.stderr) {
                this.output.appendLine(`engage list-disabled stderr: ${result.stderr.trim()}`);
            }

            const parsed = JSON.parse(result.stdout);

            if (!Array.isArray(parsed)) {
                return;
            }

            this.entries = parsed
                .filter((e): e is DisabledEntry =>
                    typeof e === "object" && e !== null &&
                    typeof e.path === "string" &&
                    typeof e.feature === "string" &&
                    typeof e.leaf === "string"
                );
        } catch (err) {
            const message = err instanceof Error ? err.message : String(err);
            this.output.appendLine(`engage list-disabled failed: ${message}`);
            this.entries = [];
        }
    }
}

function suppressedAtPosition(document: vscode.TextDocument, position: vscode.Position): boolean {
    const line = document.lineAt(position.line).text;
    const head = line.slice(0, position.character);

    // `use foo::Bar` — RA handles import paths already.
    if (/^\s*use\s/.test(line)) {
        return true;
    }

    // Doc comment or line comment.
    if (/^\s*\/\//.test(line) || /^\s*\*/.test(line)) {
        return true;
    }

    // Crude string-literal guard: an unbalanced `"` before the cursor.
    let inString = false;
    for (let i = 0; i < head.length; i++) {
        const ch = head[i];

        if (ch === "\\") {
            i += 1;
            continue;
        }

        if (ch === "\"") {
            inString = !inString;
        }
    }

    return inString;
}

function wordAtPosition(document: vscode.TextDocument, position: vscode.Position): string {
    const range = document.getWordRangeAtPosition(position);

    if (!range) {
        return "";
    }

    return document.getText(range);
}

function findUseInsertLine(document: vscode.TextDocument): number {
    let lastUseLine = -1;

    for (let i = 0; i < document.lineCount; i++) {
        const line = document.lineAt(i).text;

        if (/^\s*use\s/.test(line)) {
            lastUseLine = i;
        } else if (lastUseLine >= 0 && line.trim().length > 0 && !line.trim().startsWith("//")) {
            // Stop scanning past the import block to avoid walking the whole file.
            break;
        }
    }

    if (lastUseLine >= 0) {
        return lastUseLine + 1;
    }

    // No imports yet — insert after any inner attributes / file header.
    for (let i = 0; i < Math.min(document.lineCount, 50); i++) {
        const line = document.lineAt(i).text.trim();

        if (line.length === 0) continue;
        if (line.startsWith("//")) continue;
        if (line.startsWith("#![")) continue;

        return i;
    }

    return 0;
}

interface MethodEntry {
    method: string;
    class: string;
    path: string;
    feature: string;
    returns?: string;
    /// Names of the method's positional parameters (excluding `self`),
    /// in source order. Used to render snippet placeholders when the
    /// user accepts a completion.
    args?: string[];
    disabled: boolean;
}

/// Method-completion sibling to `EngageCompletionProvider`. Triggered when
/// the user types `.` on a Rust file and offers methods that live on a
/// disabled-feature `IXMethods` trait for the receiver's type.
///
/// Receiver inference rides on rust-analyzer's `executeTypeDefinitionProvider`:
/// for `recv.foo().bar()`, calling the provider at the receiver identifier
/// position returns the location of the type definition. We extract the
/// class name from that location and look up matching methods in the
/// CLI-emitted index.
///
/// Limitation: the index only carries methods declared on the receiver's
/// *own* `IXMethods` trait. Inherited methods (from a parent class's
/// `IXMethods`) aren't currently surfaced. The user's existing fallback
/// for that is to type the method out, see the `E0599` diagnostic, and use
/// the existing engage Quick Fix, which enables the receiver's methods
/// feature (the cascade then activates ancestor methods features too).
export class EngageMethodCompletionProvider implements vscode.CompletionItemProvider {
    private readonly context: vscode.ExtensionContext;
    private readonly output: vscode.OutputChannel;
    private methods: MethodEntry[] = [];
    private disabledByClass: Map<string, MethodEntry[]> = new Map();
    /// Count of each method's return-type leaf across the index. When the
    /// chain-walk hits an ambiguous method name (e.g., `get_transform`
    /// returns `Transform` on most classes but `Matrix4x4` on one), pick
    /// the most common — it almost always matches what the user means.
    private returnCountsByMethod: Map<string, Map<string, number>> = new Map();
    private loadPromise: Promise<void> | undefined;

    constructor(context: vscode.ExtensionContext, output: vscode.OutputChannel) {
        this.context = context;
        this.output = output;
    }

    refresh(): void {
        this.methods = [];
        this.disabledByClass = new Map();
        this.returnCountsByMethod = new Map();
        this.loadPromise = undefined;
    }

    async provideCompletionItems(
        document: vscode.TextDocument,
        position: vscode.Position,
        _token: vscode.CancellationToken,
        _context: vscode.CompletionContext,
    ): Promise<vscode.CompletionList<vscode.CompletionItem> | undefined> {
        const line = document.lineAt(position.line).text;
        const beforeCursor = line.slice(0, position.character);
        const dotIdx = beforeCursor.lastIndexOf(".");

        if (dotIdx < 0) {
            return undefined;
        }

        const afterDot = beforeCursor.slice(dotIdx + 1);

        if (!/^[a-zA-Z0-9_]*$/.test(afterDot)) {
            return undefined;
        }

        await this.ensureLoaded();

        if (this.methods.length === 0) {
            return undefined;
        }

        // Receiver inference, in order of trustworthiness:
        //   1. rust-analyzer's executeTypeDefinitionProvider — exact when
        //      every step before the trailing `.` is on an enabled feature.
        //   2. Chain-walk the receiver expression: the receiver of `.X` is
        //      whatever the immediately preceding method call returns. We
        //      have those return types in the index, so a unique
        //      method-name → return-type mapping resolves the receiver.
        let className = await this.inferReceiverClassViaRA(document, line, dotIdx, position.line);

        if (!className) {
            className = this.inferReceiverClassFromChain(document, position, dotIdx);
        }

        if (!className) {
            return undefined;
        }

        const matches = this.disabledByClass.get(className);

        if (!matches || matches.length === 0) {
            return undefined;
        }

        return this.buildItems(matches, afterDot, className);
    }

    /// Walk back from the trailing dot to find an immediately preceding
    /// `methodName(...)`. Crosses newlines so multi-line builder chains
    /// (`a\n    .foo()\n    .bar()\n    .X`) work. If exactly one
    /// return-type is associated with that method across the whole index,
    /// use it as the receiver class. Ambiguous method names (different
    /// return types per receiver) abort the inference rather than guess.
    private inferReceiverClassFromChain(
        document: vscode.TextDocument,
        position: vscode.Position,
        dotIdx: number,
    ): string | undefined {
        const text = document.getText();
        const dotOffset = document.offsetAt(new vscode.Position(position.line, dotIdx));

        let pos = dotOffset - 1;

        // Skip whitespace (spaces, tabs, newlines) leading up to the
        // previous expression.
        while (pos >= 0 && /\s/.test(text[pos])) {
            pos -= 1;
        }

        if (pos < 0 || text[pos] !== ")") {
            return undefined;
        }

        // Walk back through balanced parens to find the matching `(`.
        let depth = 0;
        let parenStart = -1;

        for (let i = pos; i >= 0; i--) {
            const ch = text[i];

            if (ch === ")") {
                depth += 1;
            } else if (ch === "(") {
                depth -= 1;

                if (depth === 0) {
                    parenStart = i;
                    break;
                }
            }
        }

        if (parenStart < 0) {
            return undefined;
        }

        let nameStart = parenStart;
        while (nameStart > 0 && /[a-zA-Z0-9_]/.test(text[nameStart - 1])) {
            nameStart -= 1;
        }

        const methodName = text.slice(nameStart, parenStart);

        if (!methodName) {
            return undefined;
        }

        const counts = this.returnCountsByMethod.get(methodName);

        if (!counts || counts.size === 0) {
            return undefined;
        }

        let bestClass: string | undefined;
        let bestCount = 0;

        for (const [cls, count] of counts) {
            if (count > bestCount) {
                bestCount = count;
                bestClass = cls;
            }
        }

        return bestClass;
    }

    private async inferReceiverClassViaRA(
        document: vscode.TextDocument,
        line: string,
        dotIdx: number,
        lineIdx: number,
    ): Promise<string | undefined> {
        const receiverPos = findReceiverIdentifierPosition(line, dotIdx, lineIdx);

        if (!receiverPos) {
            return undefined;
        }

        let locs: (vscode.Location | vscode.LocationLink)[] | undefined;

        try {
            locs = await vscode.commands.executeCommand(
                "vscode.executeTypeDefinitionProvider",
                document.uri,
                receiverPos,
            );
        } catch {
            return undefined;
        }

        if (!locs || locs.length === 0) {
            return undefined;
        }

        const loc = locs[0];
        const targetUri = "targetUri" in loc ? loc.targetUri : (loc as vscode.Location).uri;
        const targetRange = "targetSelectionRange" in loc && loc.targetSelectionRange
            ? loc.targetSelectionRange
            : ("targetRange" in loc ? loc.targetRange : (loc as vscode.Location).range);

        try {
            const targetDoc = await vscode.workspace.openTextDocument(targetUri);
            const className = targetDoc.getText(targetRange).trim();
            return className || undefined;
        } catch {
            return undefined;
        }
    }

    private buildItems(matches: MethodEntry[], afterDot: string, receiverClass: string): vscode.CompletionList<vscode.CompletionItem> | undefined {
        const typedLower = afterDot.toLowerCase();
        const items: vscode.CompletionItem[] = [];

        for (const entry of matches) {
            if (typedLower.length > 0 && !entry.method.toLowerCase().startsWith(typedLower)) {
                continue;
            }

            const item = new vscode.CompletionItem(entry.method, vscode.CompletionItemKind.Method);
            item.detail = `engage · ${receiverClass} · ${entry.feature}`;
            item.documentation = new vscode.MarkdownString(
                `Method on \`${entry.class}\` (\`${entry.path}\`).\n\nAccepting enables feature \`${entry.feature}\` in \`Cargo.toml\`.`,
            );
            // Insert `methodName(arg1, arg2, …)` with the args as tabstop
            // placeholders so the user can tab through and fill them in.
            // Cursor lands at `$0` after the closing paren so `.` chains
            // start a fresh call without any extra keypresses.
            item.insertText = buildMethodSnippet(entry.method, entry.args ?? []);
            item.command = {
                command: "engage.addFeature",
                title: "Add engage feature",
                arguments: [entry.feature],
            };
            item.sortText = `~engage~${entry.method}`;
            items.push(item);

            if (items.length >= 100) {
                break;
            }
        }

        if (items.length === 0) {
            return undefined;
        }

        return new vscode.CompletionList(items, true);
    }

    private ensureLoaded(): Promise<void> {
        if (this.loadPromise) {
            return this.loadPromise;
        }

        this.loadPromise = this.load();
        return this.loadPromise;
    }

    private async load(): Promise<void> {
        const binary = resolveBinary(this.context);
        const cwd = workspaceCwd();

        if (!cwd) {
            return;
        }

        try {
            const result = await runEngage(binary, ["features", "list-disabled-methods", "--json"], cwd);

            if (result.stderr) {
                this.output.appendLine(`engage list-disabled-methods stderr: ${result.stderr.trim()}`);
            }

            const parsed = JSON.parse(result.stdout);

            if (!Array.isArray(parsed)) {
                return;
            }

            this.methods = parsed.filter((e): e is MethodEntry =>
                typeof e === "object" && e !== null &&
                typeof e.method === "string" &&
                typeof e.class === "string" &&
                typeof e.path === "string" &&
                typeof e.feature === "string" &&
                typeof e.disabled === "boolean"
            );

            this.disabledByClass = new Map();
            this.returnCountsByMethod = new Map();

            for (const m of this.methods) {
                if (m.disabled) {
                    let bucket = this.disabledByClass.get(m.class);
                    if (!bucket) {
                        bucket = [];
                        this.disabledByClass.set(m.class, bucket);
                    }
                    bucket.push(m);
                }

                if (m.returns) {
                    let counts = this.returnCountsByMethod.get(m.method);
                    if (!counts) {
                        counts = new Map();
                        this.returnCountsByMethod.set(m.method, counts);
                    }
                    counts.set(m.returns, (counts.get(m.returns) ?? 0) + 1);
                }
            }
        } catch (err) {
            const message = err instanceof Error ? err.message : String(err);
            this.output.appendLine(`engage list-disabled-methods failed: ${message}`);
            this.methods = [];
            this.disabledByClass = new Map();
            this.returnCountsByMethod = new Map();
        }
    }
}

function buildMethodSnippet(name: string, args: string[]): vscode.SnippetString {
    if (args.length === 0) {
        return new vscode.SnippetString(`${name}()$0`);
    }

    const placeholders = args
        .map((arg, idx) => `\${${idx + 1}:${arg}}`)
        .join(", ");

    return new vscode.SnippetString(`${name}(${placeholders})$0`);
}

/// Walk back from the `.` to find an identifier start position. Skips a
/// trailing `()` or `(...)` if the receiver is a method call result —
/// rust-analyzer's type-definition lookup works fine when invoked on the
/// last identifier of the receiver expression.
function findReceiverIdentifierPosition(line: string, dotIdx: number, lineIdx: number): vscode.Position | undefined {
    let end = dotIdx;

    // Skip a trailing balanced (...) before the dot, e.g., `.foo().bar()`.
    if (end > 0 && line[end - 1] === ")") {
        let depth = 0;

        while (end > 0) {
            const ch = line[end - 1];

            if (ch === ")") depth += 1;
            else if (ch === "(") depth -= 1;

            end -= 1;

            if (depth === 0) {
                break;
            }
        }
    }

    let start = end;
    while (start > 0 && /[a-zA-Z0-9_]/.test(line[start - 1])) {
        start -= 1;
    }

    if (start === end) {
        return undefined;
    }

    return new vscode.Position(lineIdx, start);
}

export function registerCompletions(context: vscode.ExtensionContext, output: vscode.OutputChannel): void {
    const typeProvider = new EngageCompletionProvider(context, output);
    const methodProvider = new EngageMethodCompletionProvider(context, output);

    context.subscriptions.push(
        vscode.languages.registerCompletionItemProvider({ scheme: "file", language: "rust" }, typeProvider),
        vscode.languages.registerCompletionItemProvider(
            { scheme: "file", language: "rust" },
            methodProvider,
            ".",
        ),
    );

    context.subscriptions.push(
        vscode.workspace.onDidSaveTextDocument((doc) => {
            if (path.basename(doc.fileName) === "Cargo.toml") {
                typeProvider.refresh();
                methodProvider.refresh();
            }
        }),
    );
}
