import { Check, Copy } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useCopied } from "@/hooks/useCopied";

export function TranscriptBox({ label, text }: { label: string; text: string }) {
  const { copied, copy } = useCopied();
  return (
    <div className="flex flex-col gap-1.5">
      <div className="text-xs font-semibold text-muted-foreground">{label}</div>
      <div className="flex items-start gap-2 rounded-lg bg-muted py-2 pr-2 pl-3 text-sm select-text">
        <span className="flex-1 py-0.5 whitespace-pre-wrap">{text}</span>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label="Copy"
          className="text-muted-foreground"
          onClick={() => copy(text)}
        >
          {copied ? <Check /> : <Copy />}
        </Button>
      </div>
    </div>
  );
}
