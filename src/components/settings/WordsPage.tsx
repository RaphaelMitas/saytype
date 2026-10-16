import { cn } from "cn";
import { LoaderCircle, Mic, Play, Square, Volume2, X } from "lucide-react";
import { type FormEvent, useEffect, useRef, useState } from "react";
import { PageHeader } from "@/components/settings/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import type { NetworkMode } from "@/lib/network";
import {
  addWord,
  cancelExampleRecording,
  deleteExample,
  examplePrompt,
  finishExampleRecording,
  getVocabStatus,
  getWords,
  playExample,
  removeWord,
  retryTraining,
  setSayAs,
  speakWord,
  startExampleRecording,
  type VocabStatus,
  type WordView,
} from "@/lib/words";

interface CardView {
  title: string;
  detail: string;
  dot?: string;
  busy?: boolean;
  progress?: number;
  retry?: boolean;
}

function statusView(status: VocabStatus, words: WordView[]): CardView {
  const pending = words.filter((w) => !w.learned).length;
  const learning = pending === 1 ? "Learning 1 new word" : `Learning ${pending} new words`;
  switch (status.state) {
    case "waiting":
      return {
        busy: true,
        title: "About to learn your changes",
        detail: "Starts shortly, or right away when you close Settings.",
      };
    case "training":
      return {
        busy: true,
        title: pending > 0 ? learning : "Learning from your examples",
        detail: "Dictation keeps working, a bit slower until this finishes.",
        progress: status.progress,
      };
    case "failed":
      return {
        dot: "bg-destructive",
        title: "Couldn't learn your words",
        detail: `${status.message.replace(/\.$/, "")}. The last learned words still apply.`,
        retry: true,
      };
    default:
      return words.length === 0
        ? {
            dot: "bg-muted-foreground",
            title: "No words yet",
            detail: "Saytype learns each word on this Mac in about a minute.",
          }
        : {
            dot: "bg-success",
            title: "Up to date",
            detail: words.length === 1 ? "1 word" : `${words.length} words`,
          };
  }
}

function StatusCard({ view }: { view: CardView }) {
  return (
    <div className="flex flex-col gap-2.5 rounded-xl border bg-card px-4 py-3">
      <div className="flex items-center gap-3">
        {view.busy ? (
          <LoaderCircle className="size-3.5 shrink-0 animate-spin text-primary" />
        ) : (
          <span className={cn("size-2 shrink-0 rounded-full", view.dot)} />
        )}
        <div className="flex-1">
          <div className="text-sm font-semibold">{view.title}</div>
          <div className="text-xs text-muted-foreground">{view.detail}</div>
        </div>
        {view.progress !== undefined && (
          <span className="text-xs text-muted-foreground tabular-nums">
            {Math.round(view.progress * 100)}%
          </span>
        )}
        {view.retry && (
          <Button variant="outline" size="sm" onClick={() => retryTraining().catch(console.error)}>
            Retry
          </Button>
        )}
      </div>
      {view.progress !== undefined && (
        <div className="h-1 overflow-hidden rounded-full bg-muted">
          <div
            className="h-full rounded-full bg-primary transition-[width]"
            style={{ width: `${view.progress * 100}%` }}
          />
        </div>
      )}
    </div>
  );
}

function WordChip({
  word,
  selected,
  learning,
  onSelect,
  onRemove,
}: {
  word: WordView;
  selected: boolean;
  learning: boolean;
  onSelect: () => void;
  onRemove: () => void;
}) {
  return (
    <span
      className={cn(
        "inline-flex h-7 items-center rounded-full border bg-card text-sm",
        !word.learned && "border-dashed text-muted-foreground",
        selected && "border-primary bg-primary text-primary-foreground",
      )}
    >
      <button
        type="button"
        onClick={onSelect}
        className="flex h-full items-center gap-1.5 rounded-l-full pl-3 pr-1"
      >
        {!word.learned && learning && <LoaderCircle className="size-3 animate-spin" />}
        {word.word}
      </button>
      <button
        type="button"
        aria-label={`Remove ${word.word}`}
        onClick={onRemove}
        className={cn(
          "flex h-full items-center rounded-r-full pr-2 pl-1 text-muted-foreground hover:text-foreground",
          selected && "text-primary-foreground/70 hover:text-primary-foreground",
        )}
      >
        <X className="size-3" />
      </button>
    </span>
  );
}

function ExampleRecorder({
  word,
  onSaved,
}: {
  word: string;
  onSaved: (words: WordView[]) => void;
}) {
  const [attempt, setAttempt] = useState(0);
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const recordingRef = useRef(false);
  const mounted = useRef(false);
  const prompt = examplePrompt(word, attempt);

  // Leaving the page mid-recording would otherwise keep the microphone open
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      if (recordingRef.current) cancelExampleRecording().catch(console.error);
    };
  }, []);

  // Closing Settings stops the recording on the Rust side
  useTauriEvent("example-recording-cancelled", () => {
    recordingRef.current = false;
    setRecording(false);
  });

  const start = async () => {
    setError(null);
    try {
      await startExampleRecording();
      if (!mounted.current) {
        await cancelExampleRecording();
        return;
      }
      recordingRef.current = true;
      setRecording(true);
    } catch (e) {
      setError(String(e));
    }
  };

  const stop = async () => {
    recordingRef.current = false;
    setRecording(false);
    try {
      onSaved(await finishExampleRecording(prompt));
      setAttempt((a) => a + 1);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="flex flex-col gap-2">
      {recording ? (
        <div className="flex items-center gap-3 rounded-lg bg-muted py-2 pr-2 pl-3">
          <span className="size-2 shrink-0 animate-pulse rounded-full bg-destructive" />
          <span className="flex-1 text-sm">{prompt}</span>
          <Button size="sm" onClick={stop}>
            <Square />
            Done
          </Button>
        </div>
      ) : (
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={start}>
            <Mic />
            Record an example
          </Button>
          <span className="text-xs text-muted-foreground">You'll read one short sentence.</span>
        </div>
      )}
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}

function WordDetail({ word, onChange }: { word: WordView; onChange: (words: WordView[]) => void }) {
  const [sayAs, setSayAsDraft] = useState(word.say_as ?? "");
  const [error, setError] = useState<string | null>(null);
  const card = useRef<HTMLDivElement>(null);

  useEffect(() => {
    card.current?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, []);

  const report = (e: unknown) => setError(String(e));

  const update = (action: Promise<WordView[]>) => {
    setError(null);
    action.then(onChange).catch(report);
  };

  const saveSayAs = () => {
    const next = sayAs.trim() || null;
    if (next !== word.say_as) update(setSayAs(word.word, next));
  };

  return (
    <div ref={card} className="flex flex-col gap-3 rounded-xl border bg-card p-4">
      <div className="flex items-center gap-2">
        <span className="flex-1 font-semibold">{word.word}</span>
        <Button variant="ghost" size="sm" onClick={() => speakWord(word.word).catch(report)}>
          <Volume2 />
          Hear it
        </Button>
      </div>

      <div className="flex items-center gap-3">
        <Label htmlFor="say-as" className="w-24 shrink-0 text-xs font-normal text-muted-foreground">
          Pronounce as
        </Label>
        <Input
          id="say-as"
          value={sayAs}
          placeholder="Only if it sounds wrong, like “flats bee”"
          onChange={(e) => setSayAsDraft(e.target.value)}
          onBlur={saveSayAs}
          onKeyDown={(e) => {
            if (e.key === "Enter") saveSayAs();
          }}
          className="h-7"
        />
      </div>

      <div className="flex flex-col gap-2">
        <div className="text-xs font-semibold text-muted-foreground">
          Your voice
          {word.examples.length > 0 &&
            ` · ${word.examples.length} ${word.examples.length === 1 ? "example" : "examples"}`}
        </div>
        {word.examples.map((example) => (
          <div key={example.id} className="flex items-center gap-2 text-sm">
            <Button
              variant="outline"
              size="icon-xs"
              aria-label="Play"
              className="rounded-full"
              onClick={() => playExample(example.id).catch(report)}
            >
              <Play />
            </Button>
            <span className="flex-1 truncate">“{example.text}”</span>
            <Badge variant="secondary">
              {example.source === "history" ? "from History" : "recorded"}
            </Badge>
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label="Delete example"
              className="text-muted-foreground"
              onClick={() => update(deleteExample(example.id))}
            >
              <X />
            </Button>
          </div>
        ))}
        <ExampleRecorder word={word.word} onSaved={onChange} />
      </div>
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}

export function WordsPage({ mode }: { mode: NetworkMode }) {
  const [words, setWords] = useState<WordView[] | null>(null);
  const [status, setStatus] = useState<VocabStatus>({ state: "idle" });
  const [selected, setSelected] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const client = mode === "client_only";

  const reload = () => {
    getWords().then(setWords).catch(console.error);
  };

  useEffect(() => {
    if (client) return;
    getWords().then(setWords).catch(console.error);
    getVocabStatus().then(setStatus).catch(console.error);
  }, [client]);

  useTauriEvent("words-changed", reload);
  useTauriEvent<VocabStatus>("vocab-status", setStatus);

  const add = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    try {
      const next = await addWord(draft);
      setWords(next);
      setDraft("");
      setSelected(next[next.length - 1]?.word ?? null);
    } catch (e) {
      setError(String(e));
    }
  };

  const remove = async (word: string) => {
    setError(null);
    try {
      setWords(await removeWord(word));
      if (selected === word) setSelected(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const header = (
    <PageHeader title="Words" description="Names and terms Saytype should spell your way." />
  );

  if (client) {
    return (
      <>
        {header}
        <StatusCard
          view={{
            dot: "bg-primary",
            title: "Your server's words apply",
            detail: "Transcription runs on your Saytype server, so words are added there.",
          }}
        />
      </>
    );
  }

  if (!words) return header;

  const learning = status.state === "waiting" || status.state === "training";
  const selectedWord = words.find((w) => w.word === selected);

  return (
    <>
      {header}
      <StatusCard view={statusView(status, words)} />

      <form onSubmit={add} className="flex flex-col gap-1.5">
        <div className="flex gap-2">
          <Input
            value={draft}
            placeholder="Add a word, like Kimeta"
            onChange={(e) => setDraft(e.target.value)}
          />
          <Button type="submit" disabled={!draft.trim()}>
            Add
          </Button>
        </div>
        {error && <p className="text-xs text-destructive">{error}</p>}
      </form>

      {words.length > 0 && (
        <div className="flex flex-wrap gap-1.5 rounded-xl border bg-card p-4">
          {words.map((word) => (
            <WordChip
              key={word.word}
              word={word}
              selected={word.word === selected}
              learning={learning}
              onSelect={() => setSelected(word.word === selected ? null : word.word)}
              onRemove={() => remove(word.word)}
            />
          ))}
        </div>
      )}

      {selectedWord ? (
        <WordDetail key={selectedWord.word} word={selectedWord} onChange={setWords} />
      ) : (
        words.length > 0 && (
          <p className="text-xs text-muted-foreground">
            Select a word to hear how Saytype says it or to add your own voice.
          </p>
        )
      )}
    </>
  );
}
