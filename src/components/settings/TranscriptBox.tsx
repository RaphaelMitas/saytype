import { Check, Copy } from "lucide-react";
import { Section } from "@/components/settings/Section";
import { Button } from "@/components/ui/button";
import { useCopied } from "@/hooks/useCopied";

export function TranscriptBox({ label, text }: { label: string; text: string }) {
  const { copied, copy } = useCopied();
  return (
    <Section label={label}>
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
    </Section>
  );
}
