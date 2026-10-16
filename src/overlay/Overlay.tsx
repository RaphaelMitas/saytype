import { cn } from "cn";
import { CircleAlert, CircleCheck, LoaderCircle } from "lucide-react";
import { useEffect, useLayoutEffect, useReducer, useRef, useState } from "react";
import { Kbd } from "@/components/ui/kbd";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import type { PreviewStyle } from "@/lib/preview";

type OverlayEvent =
  | { phase: "listening"; style: PreviewStyle; hotkey: string }
  | { phase: "live"; text: string }
  | { phase: "finishing" }
  | { phase: "pasted"; text: string }
  | { phase: "empty" }
  | { phase: "failed"; message: string }
  | { phase: "hidden" };

interface View {
  phase: Exclude<OverlayEvent["phase"], "hidden">;
  visible: boolean;
  style: PreviewStyle;
  hotkey: string;
  startedAt: number;
  settled: string;
  unsettled: string;
  lastWords: string[];
  message: string;
}

const INITIAL: View = {
  phase: "empty",
  visible: false,
  style: "panel",
  hotkey: "",
  startedAt: 0,
  settled: "",
  unsettled: "",
  lastWords: [],
  message: "",
};

const HUD = "border border-white/10 bg-zinc-900/95 text-zinc-100 shadow-lg shadow-black/40";
const BAR_SHAPE = [0.55, 0.85, 1, 0.75, 0.5];

const isRecording = (phase: View["phase"]) => phase === "listening" || phase === "live";

const normalize = (word: string) => word.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "");

function sharedPrefix(a: string[], b: string[]): number {
  let i = 0;
  while (i < a.length && i < b.length && normalize(a[i]) === normalize(b[i])) i++;
  return i;
}

function reduce(view: View, event: OverlayEvent): View {
  switch (event.phase) {
    case "listening":
      return {
        ...INITIAL,
        phase: "listening",
        visible: true,
        style: event.style,
        hotkey: event.hotkey,
        startedAt: Date.now(),
      };
    case "live": {
      // A pass that was in flight on release can land after "finishing"
      if (!isRecording(view.phase)) return view;
      const words = event.text.split(/\s+/).filter(Boolean);
      // Words the last two passes agree on are unlikely to change again
      const settled = sharedPrefix(view.lastWords, words);
      return {
        ...view,
        phase: "live",
        settled: words.slice(0, settled).join(" "),
        unsettled: words.slice(settled).join(" "),
        lastWords: words,
      };
    }
    case "pasted":
      return { ...view, phase: "pasted", settled: event.text, unsettled: "" };
    case "failed":
      return { ...view, phase: "failed", message: event.message };
    case "hidden":
      // Keep the outcome on screen while it fades out
      return { ...view, visible: false };
    default:
      return { ...view, phase: event.phase };
  }
}

function useElapsedSeconds(startedAt: number, running: boolean): number {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (!running) return;
    setNow(Date.now());
    const id = setInterval(() => setNow(Date.now()), 250);
    return () => clearInterval(id);
  }, [running]);
  return Math.max(0, Math.floor((now - startedAt) / 1000));
}

function useOverflow(content: string) {
  const outer = useRef<HTMLDivElement>(null);
  const inner = useRef<HTMLSpanElement>(null);
  const [overflowing, setOverflowing] = useState(false);
  // biome-ignore lint/correctness/useExhaustiveDependencies: re-measure whenever the text changes
  useLayoutEffect(() => {
    if (!outer.current || !inner.current) return;
    setOverflowing(
      inner.current.offsetWidth > outer.current.clientWidth ||
        inner.current.offsetHeight > outer.current.clientHeight,
    );
  }, [content]);
  return { outer, inner, overflowing };
}

function Meter({ level }: { level: number }) {
  return (
    <div className="flex h-4 items-center gap-0.5">
      {BAR_SHAPE.map((shape) => (
        <span
          key={shape}
          className="w-[3px] rounded-full bg-zinc-100 transition-[height] duration-75"
          style={{ height: `${Math.max(3, shape * level * 16)}px` }}
        />
      ))}
    </div>
  );
}

function RecordingDot() {
  return <span className="size-2 shrink-0 animate-pulse rounded-full bg-primary" />;
}

function Transcript({ view }: { view: View }) {
  return (
    <>
      <span>{view.settled}</span>
      {view.settled && view.unsettled && " "}
      <span className="text-zinc-100/45">{view.unsettled}</span>
    </>
  );
}

function StatusIcon({ phase }: { phase: View["phase"] }) {
  if (phase === "finishing") return <LoaderCircle className="size-3.5 animate-spin" />;
  if (phase === "pasted") return <CircleCheck className="size-3.5 text-success" />;
  return <CircleAlert className="size-3.5 text-zinc-400" />;
}

function Panel({ view, level }: { view: View; level: number }) {
  const recording = isRecording(view.phase);
  const elapsed = useElapsedSeconds(view.startedAt, recording);
  const hasText = Boolean(view.settled || view.unsettled);

  const placeholder =
    view.phase === "empty"
      ? "Didn't catch anything."
      : view.phase === "failed"
        ? view.message
        : hasText
          ? null
          : recording
            ? "Listening…"
            : "Transcribing…";
  const { outer, inner, overflowing } = useOverflow(placeholder ?? view.settled + view.unsettled);

  return (
    <div
      className={cn(
        "w-[480px] rounded-2xl px-3.5 pt-3 pb-2.5",
        HUD,
        view.phase === "pasted" && "border-success/50",
      )}
    >
      <div
        ref={outer}
        className={cn(
          "flex max-h-[4.35em] flex-col justify-end overflow-hidden text-[15px] leading-[1.45]",
          overflowing && "mask-[linear-gradient(to_bottom,transparent,black_1.2em)]",
        )}
      >
        <span ref={inner} className={cn(placeholder && "text-zinc-100/40")}>
          {placeholder ?? <Transcript view={view} />}
        </span>
      </div>
      <div className="mt-2 flex items-center gap-2 text-[11.5px] whitespace-nowrap text-zinc-400">
        {recording ? (
          <>
            <RecordingDot />
            <Meter level={level} />
            <span className="tabular-nums">
              {Math.floor(elapsed / 60)}:{String(elapsed % 60).padStart(2, "0")}
            </span>
            {view.hotkey && (
              <span className="ml-auto">
                release{" "}
                <Kbd className="h-4 border-white/15 bg-white/10 text-[10.5px] text-zinc-200">
                  {view.hotkey}
                </Kbd>{" "}
                to paste
              </span>
            )}
          </>
        ) : (
          <>
            <StatusIcon phase={view.phase} />
            {view.phase === "finishing"
              ? "Finishing…"
              : view.phase === "pasted"
                ? "Pasted"
                : "Nothing pasted"}
          </>
        )}
      </div>
    </div>
  );
}

function Compact({ view, level }: { view: View; level: number }) {
  const recording = isRecording(view.phase);
  // The meter gives way to a narrower spinner on release, so the text gets room
  const { outer, inner, overflowing } = useOverflow(`${recording}${view.settled}${view.unsettled}`);

  const label =
    view.phase === "pasted"
      ? "Pasted"
      : view.phase === "empty"
        ? "No speech detected"
        : view.phase === "failed"
          ? "Nothing pasted"
          : null;

  return (
    <div
      className={cn(
        "flex h-9 max-w-[440px] items-center gap-2.5 rounded-full px-3.5 text-[13.5px]",
        HUD,
      )}
    >
      {recording ? (
        <>
          <RecordingDot />
          <Meter level={level} />
        </>
      ) : (
        <StatusIcon phase={view.phase} />
      )}
      {label ? (
        <span className="text-[12.5px]">{label}</span>
      ) : view.settled || view.unsettled ? (
        <div
          ref={outer}
          className={cn(
            "flex min-w-0 justify-end overflow-hidden whitespace-nowrap",
            overflowing && "mask-[linear-gradient(to_right,transparent,black_36px)]",
          )}
        >
          <span ref={inner} className="shrink-0">
            <Transcript view={view} />
          </span>
        </div>
      ) : (
        <span className="text-[12.5px] text-zinc-100/55">
          {recording ? "Listening…" : "Transcribing…"}
        </span>
      )}
    </div>
  );
}

export function Overlay() {
  const [view, dispatch] = useReducer(reduce, INITIAL);
  const [level, setLevel] = useState(0);

  useTauriEvent<OverlayEvent>("overlay", (event) => {
    if (event.phase === "listening") setLevel(0);
    dispatch(event);
  });
  useTauriEvent<number>("overlay-level", setLevel);

  return (
    <div
      className={cn(
        "flex h-full items-end justify-center p-3 transition-opacity duration-200",
        !view.visible && "opacity-0",
      )}
    >
      {view.style === "compact" ? (
        <Compact view={view} level={level} />
      ) : (
        <Panel view={view} level={level} />
      )}
    </div>
  );
}
