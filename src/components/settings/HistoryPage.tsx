import { cn } from "cn";
import { Play, X } from "lucide-react";
import { type FormEvent, useEffect, useState } from "react";
import { PageHeader } from "@/components/settings/PageHeader";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import {
  clearHistory,
  deleteHistoryEntry,
  getHistory,
  type History,
  type HistoryEntry,
  playHistoryEntry,
  setKeepHistory,
  teachFromHistory,
} from "@/lib/history";
import type { NetworkMode } from "@/lib/network";

function timeAgo(atMs: number): string {
  const minutes = Math.floor((Date.now() - atMs) / 60_000);
  if (minutes < 1) return "now";
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} h`;
  if (hours < 48) return "yesterday";
  return new Date(atMs).toLocaleDateString(undefined, { day: "numeric", month: "short" });
}

/** Whitespace stays its own token so the line can be put back together exactly */
const tokenize = (text: string) => text.split(/(\s+)/).filter(Boolean);
const isSpace = (token: string) => /^\s+$/.test(token);

/** Splits "“Flatspy,”" into the word and the punctuation around it, which a fix keeps */
function trimPunctuation(text: string) {
  const prefix = text.match(/^[^\p{L}\p{N}]*/u)?.[0] ?? "";
  const rest = text.slice(prefix.length);
  const suffix = rest.match(/[^\p{L}\p{N}]*$/u)?.[0] ?? "";
  return { prefix, core: rest.slice(0, rest.length - suffix.length), suffix };
}

interface Fix {
  word: string;
  learn: boolean;
}

interface Draft {
  tokens: string[];
  fixes: Fix[];
  selection: [number, number] | null;
}

function FixBar({
  original,
  onFix,
  onCancel,
}: {
  original: string;
  onFix: (replacement: string, learn: boolean) => void;
  onCancel: () => void;
}) {
  const [replacement, setReplacement] = useState(original);
  const [learn, setLearn] = useState(true);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (replacement.trim()) onFix(replacement.trim(), learn);
  };

  return (
    <form onSubmit={submit} className="flex flex-col gap-2 rounded-lg bg-muted p-2.5">
      <div className="flex items-center gap-2">
        <span className="shrink-0 text-xs text-muted-foreground">“{original}” should be</span>
        <Input
          autoFocus
          value={replacement}
          onChange={(e) => setReplacement(e.target.value)}
          onFocus={(e) => e.target.select()}
          className="h-7 bg-background"
        />
        <Button type="submit" size="sm" disabled={!replacement.trim()}>
          Fix
        </Button>
        <Button type="button" variant="ghost" size="sm" onClick={onCancel}>
          Cancel
        </Button>
      </div>
      <label className="flex items-center gap-2 text-xs text-muted-foreground">
        <input type="checkbox" checked={learn} onChange={(e) => setLearn(e.target.checked)} />
        Add to my words. Leave off for ordinary words Saytype just misheard this time.
      </label>
    </form>
  );
}

function Row({
  entry,
  draft,
  canTeach,
  onSelect,
  onDraftChange,
  onDelete,
  onSaved,
}: {
  entry: HistoryEntry;
  draft: Draft | null;
  canTeach: boolean;
  onSelect: (index: number, extend: boolean) => void;
  onDraftChange: (draft: Draft | null) => void;
  onDelete: () => void;
  onSaved: (message: string | null) => void;
}) {
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const tokens = draft?.tokens ?? tokenize(entry.text);
  const [start, end] = draft?.selection ?? [-1, -1];
  const selected = draft?.selection ? trimPunctuation(tokens.slice(start, end + 1).join("")) : null;

  const applyFix = (replacement: string, learn: boolean) => {
    if (!draft || !selected) return;
    const text = [
      ...draft.tokens.slice(0, start),
      selected.prefix + replacement + selected.suffix,
      ...draft.tokens.slice(end + 1),
    ].join("");
    onDraftChange({
      ...draft,
      tokens: tokenize(text),
      fixes: [...draft.fixes, { word: replacement, learn }],
      selection: null,
    });
  };

  // A token fixed twice only teaches the spelling the line ended up with
  const learned = [
    ...new Set(
      draft?.fixes.filter((f) => f.learn && tokens.join("").includes(f.word)).map((f) => f.word),
    ),
  ];

  const save = async () => {
    if (!draft) return;
    setSaving(true);
    try {
      const { example_saved } = await teachFromHistory(entry.id, draft.tokens.join(""), learned);
      onDraftChange(null);
      onSaved(
        learned.length > 0 && !example_saved
          ? `Added ${learned.join(", ")}. This dictation is too long to use as a voice example.`
          : null,
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="group flex flex-col gap-2 border-t px-3 py-2.5 first:border-t-0">
      <div className="flex items-start gap-2.5">
        <Button
          variant="outline"
          size="icon-xs"
          aria-label="Play"
          className="mt-0.5 rounded-full"
          onClick={() => playHistoryEntry(entry.id).catch((e) => setError(String(e)))}
        >
          <Play />
        </Button>
        <p className="flex-1 text-sm leading-relaxed select-text">
          {tokens.map((token, index) =>
            isSpace(token) || !canTeach ? (
              // biome-ignore lint/suspicious/noArrayIndexKey: tokens have no identity beyond their position
              <span key={index}>{token}</span>
            ) : (
              <button
                // biome-ignore lint/suspicious/noArrayIndexKey: tokens have no identity beyond their position
                key={index}
                type="button"
                onClick={(e) => onSelect(index, e.shiftKey)}
                className={cn(
                  "rounded-sm hover:bg-muted",
                  index >= start &&
                    index <= end &&
                    "bg-primary/15 text-primary outline outline-primary",
                )}
              >
                {token}
              </button>
            ),
          )}
        </p>
        <span className="mt-0.5 text-xs text-muted-foreground">{timeAgo(entry.at_ms)}</span>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label="Delete"
          className="text-muted-foreground opacity-0 group-hover:opacity-100"
          onClick={onDelete}
        >
          <X />
        </Button>
      </div>

      {draft && selected && (
        <FixBar
          key={`${start}-${end}`}
          original={selected.core}
          onFix={applyFix}
          onCancel={() => onDraftChange({ ...draft, selection: null })}
        />
      )}

      {draft && draft.fixes.length > 0 && !draft.selection && (
        <div className="flex items-center gap-2 rounded-lg bg-muted p-2.5">
          <span className="flex-1 text-xs text-muted-foreground">
            {learned.length > 0
              ? `Saves this line and teaches ${learned.join(", ")} with your recording. Fix every wrong word first.`
              : "Saves the corrected line. Nothing is added to your words."}
          </span>
          <Button size="sm" disabled={saving} onClick={save}>
            Save
          </Button>
          <Button variant="ghost" size="sm" onClick={() => onDraftChange(null)}>
            Discard
          </Button>
        </div>
      )}
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}

export function HistoryPage({ mode }: { mode: NetworkMode }) {
  const [history, setHistory] = useState<History | null>(null);
  // Per row, so starting a fix in one line keeps the unsaved fixes in another
  const [drafts, setDrafts] = useState<Record<string, Draft>>({});
  const [notice, setNotice] = useState<string | null>(null);
  const canTeach = mode !== "client_only";

  const reload = () => {
    getHistory().then(setHistory).catch(console.error);
  };

  useEffect(reload, []);
  useTauriEvent("history-changed", reload);

  const select = (entry: HistoryEntry, index: number, extend: boolean) => {
    setNotice(null);
    const current = drafts[entry.id];
    const anchor = current?.selection?.[0];
    const selection: [number, number] =
      extend && anchor !== undefined
        ? [Math.min(anchor, index), Math.max(anchor, index)]
        : [index, index];
    setDraft(entry.id, {
      tokens: current?.tokens ?? tokenize(entry.text),
      fixes: current?.fixes ?? [],
      selection,
    });
  };

  const setDraft = (id: string, draft: Draft | null) => {
    setDrafts(({ [id]: _, ...rest }) => (draft ? { ...rest, [id]: draft } : rest));
  };

  const run = (action: Promise<void>) => {
    action.then(reload).catch(console.error);
  };

  const header = (
    <PageHeader
      title="History"
      description={
        canTeach
          ? "Recent dictations. Click a misheard word to teach it."
          : "Recent dictations. Words are taught on your Saytype server."
      }
    />
  );

  if (!history) return header;

  if (!history.keep) {
    return (
      <>
        {header}
        <div className="flex items-center gap-3 rounded-xl border bg-card px-4 py-3">
          <div className="flex-1">
            <div className="text-sm font-semibold">History is off</div>
            <div className="text-xs text-muted-foreground">
              Dictations and their audio aren't kept after they're pasted.
            </div>
          </div>
          <Button variant="outline" size="sm" onClick={() => run(setKeepHistory(true))}>
            Turn On
          </Button>
        </div>
      </>
    );
  }

  return (
    <>
      {header}
      {notice && <p className="text-xs text-muted-foreground">{notice}</p>}
      {history.entries.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          Nothing yet. Your dictations show up here after they're pasted.
        </p>
      ) : (
        <div className="flex flex-col rounded-xl border bg-card">
          {history.entries.map((entry) => (
            <Row
              key={entry.id}
              entry={entry}
              draft={drafts[entry.id] ?? null}
              canTeach={canTeach}
              onSelect={(index, extend) => select(entry, index, extend)}
              onDraftChange={(draft) => setDraft(entry.id, draft)}
              onDelete={() => run(deleteHistoryEntry(entry.id))}
              onSaved={setNotice}
            />
          ))}
        </div>
      )}
      <div className="flex items-center gap-1">
        <p className="flex-1 text-xs text-muted-foreground">
          Your last 50 dictations with audio, kept only on this Mac.
        </p>
        {history.entries.length > 0 && (
          <Button variant="ghost" size="sm" onClick={() => run(clearHistory())}>
            Clear
          </Button>
        )}
        <Button variant="ghost" size="sm" onClick={() => run(setKeepHistory(false))}>
          Turn Off
        </Button>
      </div>
    </>
  );
}
