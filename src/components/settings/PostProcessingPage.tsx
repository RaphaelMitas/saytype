import { cn } from "cn";
import { ChevronDown, ChevronUp, Plus } from "lucide-react";
import { useCallback, useRef, useState } from "react";
import { PageHeader } from "@/components/settings/PageHeader";
import { Section } from "@/components/settings/Section";
import { TranscriptBox } from "@/components/settings/TranscriptBox";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { useRefreshOnFocus } from "@/hooks/useRefreshOnFocus";
import {
  type Availability,
  deletePrompt,
  getPostProcessing,
  type PostProcessing,
  type Prompt,
  savePrompt,
  selectPrompt,
  setPostProcessingEnabled,
  type TestResult,
  testPostProcessing,
} from "@/lib/postProcessing";

const COLLAPSED_ROWS = 3;

const AVAILABILITY: Record<Availability, { dot: string; hint?: string }> = {
  available: { dot: "bg-success" },
  model_not_ready: {
    dot: "bg-warning",
    hint: "Apple Intelligence is still downloading. Raw text is pasted until it's ready.",
  },
  apple_intelligence_not_enabled: {
    dot: "bg-muted-foreground",
    hint: "Turn on Apple Intelligence in System Settings to use it.",
  },
  device_not_eligible: {
    dot: "bg-muted-foreground",
    hint: "This Mac doesn't support Apple Intelligence.",
  },
  unsupported: { dot: "bg-muted-foreground", hint: "Apple Intelligence needs macOS 26 or later." },
};

interface Draft {
  id: string | null;
  name: string;
  text: string;
}

function collapsedPrompts(prompts: Prompt[], selectedId: string): Prompt[] {
  const byPriority = [...prompts].sort(
    (a, b) =>
      Number(b.id === selectedId) - Number(a.id === selectedId) ||
      Number(a.builtin) - Number(b.builtin),
  );
  const shown = new Set(byPriority.slice(0, COLLAPSED_ROWS));
  return prompts.filter((prompt) => shown.has(prompt));
}

interface EditorProps {
  draft: Draft;
  builtin: boolean;
  onChange: (draft: Draft) => void;
  onCancel: () => void;
  onDone: () => void;
}

function Editor({ draft, builtin, onChange, onCancel, onDone }: EditorProps) {
  return (
    <div className="flex flex-col gap-2">
      {!builtin && (
        <Input
          autoFocus
          placeholder="Name"
          value={draft.name}
          onChange={(e) => onChange({ ...draft, name: e.target.value })}
        />
      )}
      <Textarea
        autoFocus={builtin}
        placeholder="Tell the model how to rewrite the transcript."
        className="font-mono text-xs"
        value={draft.text}
        onChange={(e) => onChange({ ...draft, text: e.target.value })}
      />
      <div className="flex justify-end gap-1">
        <Button variant="ghost" size="xs" className="text-muted-foreground" onClick={onCancel}>
          Cancel
        </Button>
        <Button
          variant="outline"
          size="xs"
          disabled={!draft.text.trim() || (!builtin && !draft.name.trim())}
          onClick={onDone}
        >
          Done
        </Button>
      </div>
    </div>
  );
}

export function PostProcessingPage() {
  const [state, setState] = useState<PostProcessing | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [sample, setSample] = useState("");
  const [result, setResult] = useState<TestResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);

  // Refreshes can resolve out of order, and only the latest one reflects the config
  const latest = useRef(0);
  const refresh = useCallback(() => {
    const request = ++latest.current;
    return getPostProcessing()
      .then((next) => request === latest.current && setState(next))
      .catch(console.error);
  }, []);

  useRefreshOnFocus(refresh);

  if (!state) return null;

  const availability = AVAILABILITY[state.availability];
  const usable = state.availability === "available" || state.availability === "model_not_ready";
  const selected = state.selected;
  const canTest =
    usable && sample.trim() !== "" && !testing && (!draft || draft.text.trim() !== "");
  const visible = expanded ? state.prompts : collapsedPrompts(state.prompts, selected.id);

  const run = (action: Promise<unknown>) => action.then(refresh).catch(console.error);

  const editor = (builtin: boolean) =>
    draft && (
      <Editor
        draft={draft}
        builtin={builtin}
        onChange={setDraft}
        onCancel={() => setDraft(null)}
        onDone={() =>
          run(savePrompt(draft.id, draft.name.trim(), draft.text.trim()).then(() => setDraft(null)))
        }
      />
    );

  const handleTest = async () => {
    setTesting(true);
    setError(null);
    try {
      setResult(await testPostProcessing(draft?.text ?? selected.text, sample));
    } catch (e) {
      setResult(null);
      setError(String(e));
    } finally {
      setTesting(false);
    }
  };

  return (
    <>
      <PageHeader
        title="Post-processing"
        description="Rewrite each transcript with a language model before it's pasted."
      />

      <div className="flex flex-col gap-3 rounded-xl border bg-card p-4">
        <div className="flex items-center gap-3">
          <div className="flex-1">
            <div className="text-sm font-semibold">Post-process transcripts</div>
            <div className="text-xs text-muted-foreground">
              Raw text is pasted if the model fails or takes longer than {state.timeout_secs}
              &nbsp;s.
            </div>
          </div>
          <Switch
            checked={state.enabled}
            disabled={!usable && !state.enabled}
            onCheckedChange={(enabled) => run(setPostProcessingEnabled(enabled))}
          />
        </div>
        <div className="-mx-4 border-t" />
        <div className="flex items-center gap-3">
          <div className="flex-1 text-sm font-medium">Model</div>
          <div className="relative w-56">
            <span
              className={cn(
                "pointer-events-none absolute top-1/2 left-2.5 size-2 -translate-y-1/2 rounded-full",
                availability.dot,
              )}
            />
            <select
              defaultValue={state.model}
              className="h-8 w-full appearance-none rounded-lg border border-input bg-transparent pr-7 pl-6 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 dark:bg-input/30"
            >
              <option value="apple_intelligence">Apple Intelligence</option>
            </select>
            <ChevronDown className="pointer-events-none absolute top-1/2 right-2 size-3.5 -translate-y-1/2 text-muted-foreground" />
          </div>
        </div>
        {availability.hint && <p className="text-xs text-muted-foreground">{availability.hint}</p>}
      </div>

      <Section label="Prompt">
        <div className="divide-y overflow-hidden rounded-lg border">
          {visible.map((prompt) => {
            const isSelected = prompt.id === selected.id;
            const isEditing = draft?.id === prompt.id;
            return (
              <div
                key={prompt.id}
                className={cn("flex flex-col gap-2 px-3 py-2", isSelected && "bg-primary/5")}
              >
                <div className="flex items-center gap-2.5">
                  <button
                    type="button"
                    className="flex flex-1 items-center gap-2.5 text-left text-sm"
                    disabled={!!draft}
                    onClick={() => !isSelected && run(selectPrompt(prompt.id))}
                  >
                    <span
                      className={cn(
                        "size-3.5 shrink-0 rounded-full border-[1.5px] border-input",
                        isSelected && "border-4 border-primary",
                      )}
                    />
                    {prompt.name}
                    {prompt.edited && <Badge variant="secondary">Edited</Badge>}
                    {!prompt.builtin && <Badge variant="secondary">Yours</Badge>}
                  </button>
                  {isEditing && (prompt.edited || !prompt.builtin) && (
                    <Button
                      variant="ghost"
                      size="xs"
                      className="text-muted-foreground"
                      onClick={() => run(deletePrompt(prompt.id).then(() => setDraft(null)))}
                    >
                      {prompt.builtin ? "Reset" : "Delete"}
                    </Button>
                  )}
                  {isSelected && !draft && (
                    <Button
                      variant="ghost"
                      size="xs"
                      className="text-muted-foreground"
                      onClick={() =>
                        setDraft({ id: prompt.id, name: prompt.name, text: prompt.text })
                      }
                    >
                      Edit
                    </Button>
                  )}
                </div>
                {isEditing && editor(prompt.builtin)}
              </div>
            );
          })}
          {draft?.id === null && <div className="px-3 py-2">{editor(false)}</div>}
          <div className="flex items-center px-3 py-1">
            {state.prompts.length > COLLAPSED_ROWS && (
              <Button
                variant="ghost"
                size="xs"
                className="-ml-2 text-muted-foreground"
                onClick={() => setExpanded(!expanded)}
              >
                {expanded ? "Show fewer" : `Show all ${state.prompts.length} prompts`}
                {expanded ? <ChevronUp /> : <ChevronDown />}
              </Button>
            )}
            <div className="flex-1" />
            {!draft && (
              <Button
                variant="ghost"
                size="xs"
                className="-mr-2 text-primary hover:text-primary"
                onClick={() => setDraft({ id: null, name: "", text: "" })}
              >
                <Plus />
                New
              </Button>
            )}
          </div>
        </div>
      </Section>

      <Section label="Try it">
        <div className="flex gap-2">
          <Input
            placeholder="um so I think we should uh ship this on friday right"
            value={sample}
            onChange={(e) => setSample(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && canTest && handleTest()}
          />
          <Button variant="outline" disabled={!canTest} onClick={handleTest}>
            {testing ? "Running…" : "Run"}
          </Button>
        </div>
        {result && <TranscriptBox label={`Result · ${result.millis} ms`} text={result.text} />}
        {error && <p className="text-xs text-destructive">{error}</p>}
      </Section>
    </>
  );
}
