import { useEffect, useRef, useState } from "react";
import { BriefCard } from "../components/app/BriefCard";
import { ChatMessage, ChatNote } from "../components/app/ChatMessage";
import { Composer } from "../components/app/Composer";
import { PageHeader } from "../components/app/PageHeader";
import { PipelineSteps } from "../components/app/PipelineSteps";
import { ReadyBox, RightPanel } from "../components/app/RightPanel";
import { RunLog, RunLogItem } from "../components/app/RunLog";
import { SessionList, SessionStatus } from "../components/app/SessionList";
import { SuggestedAnswers } from "../components/app/SuggestedAnswers";
import { Button } from "../components/ui/button";
import { Dialog } from "../components/ui/dialog";
import { notify } from "../components/ui/toast";
import { Tooltip } from "../components/ui/tooltip";
import { api, shortPath, type Project } from "../lib/api";
import {
  composerPlaceholder,
  generateTooltip,
  grillApi,
  isAdrQuestion,
  isAdrYes,
  isBriefQuestion,
  joinChoices,
  nextQuestion,
  type GrillDetail,
  type GrillMessage,
} from "../lib/grill";
import { useGrillSession, useGrillSessions } from "../lib/useGrill";
import { count, formatWhen } from "../lib/time";

// DESIGN_SYSTEM.md §7.1 screen 3, Grill (build step 3). Main = chat column (ChatMessage, the question card,
// Composer); RightPanel = SessionList + RunLog + ReadyBox. The data lives in Rust: every change arrives as a
// `grill-changed` event and the session is loaded again. Claude's text is shown as text, never as HTML.
export function GrillScreen({
  project,
  home,
  sessionId,
  artifactId,
  onOpenSession,
}: {
  project: Project;
  home: string | null;
  sessionId: number;
  artifactId: number;
  onOpenSession: (sessionId: number) => void;
}) {
  const { detail, error } = useGrillSession(sessionId);
  const sessions = useGrillSessions(artifactId);
  const [artifactName, setArtifactName] = useState("");
  const [panel, setPanel] = useState(true);
  const [text, setText] = useState("");
  // multiSelect: the chosen option indexes of the open question.
  const [chosen, setChosen] = useState<number[]>([]);
  const [busy, setBusy] = useState<"send" | "stop" | "resume" | "confirm" | "new" | "adr" | null>(null);
  // The prepared ADR dialog can be put away and opened again from the chat note.
  const [adrHidden, setAdrHidden] = useState(false);
  const [adrAsk, setAdrAsk] = useState<{ question: string; answer: string } | null>(null);
  const composer = useRef<HTMLTextAreaElement>(null);
  const chat = useRef<HTMLDivElement>(null);

  useEffect(() => {
    api.listArtifacts(project.id).then(
      (all) => setArtifactName(all.find((a) => a.id === artifactId)?.name ?? ""),
      () => setArtifactName(""),
    );
  }, [project.id, artifactId]);

  const open = nextQuestion(detail?.openBlock ?? null);
  const openKey = open ? `${detail?.openBlock?.messageId}:${open.index}` : "";
  // A newly prepared ADR opens its dialog.
  const reviewKey = detail?.adrReview?.branch ?? "";
  useEffect(() => setAdrHidden(false), [reviewKey]);
  // A new question clears the chip selection.
  useEffect(() => setChosen([]), [openKey]);
  // Newest at the bottom.
  useEffect(() => {
    const el = chat.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [detail?.messages.length, detail?.session.state]);

  if (!detail) {
    return (
      <main className="min-w-0 flex-1 rounded-panel bg-surface-1 px-10 py-8">
        {error && <ChatNote tone="error">{error}</ChatNote>}
      </main>
    );
  }

  const d = detail;
  const state = d.session.state;
  const waiting = state === "waiting_owner" || state === "brief_ready";
  const briefWaits = state === "brief_ready" && d.brief !== null && !d.brief.confirmed;
  const briefConfirmed = d.brief?.confirmed ?? false;
  const openSessionExists = sessions.some((s) => s.state !== "confirmed");
  const title = d.session.featureName ?? "New feature";

  async function act(kind: NonNullable<typeof busy>, f: () => Promise<unknown>, failTitle: string) {
    setBusy(kind);
    try {
      await f();
    } catch (e) {
      notify.error(failTitle, String(e));
    } finally {
      setBusy(null);
    }
  }

  // The answer is the Composer text. A chip fills it; for multiSelect the chosen labels are joined with ", "
  // (the Composer cannot send an empty box, so the joined answer lives in it and the owner can still edit it).
  function currentAnswer(): string {
    return text.trim();
  }

  function send() {
    const answer = currentAnswer();
    if (!answer || !waiting) return;
    if (open) {
      if (isAdrQuestion(open.question) && isAdrYes(answer)) {
        setAdrAsk({ question: open.question.question, answer });
        return;
      }
      void act("send", async () => {
        await grillApi.answer(sessionId, open.question.question, answer, false);
        setText("");
      }, "Could not send the answer");
      return;
    }
    void act("send", async () => {
      await grillApi.send(sessionId, answer);
      setText("");
    }, "Could not send the message");
  }

  function focusComposer() {
    setTimeout(() => composer.current?.focus(), 0);
  }

  // Single choice: the chip fills the Composer and does not send (§6.2 SuggestedAnswers).
  function pick(index: number) {
    const label = open?.question.options[index]?.label;
    if (label === undefined) return;
    setText(label);
    focusComposer();
  }

  function toggle(index: number) {
    const q = open?.question;
    if (!q) return;
    const next = chosen.includes(index) ? chosen.filter((i) => i !== index) : [...chosen, index].sort((a, b) => a - b);
    setChosen(next);
    setText(joinChoices(next.map((i) => q.options[i].label)));
  }

  async function newGrill() {
    const versions = await api.listVersions(artifactId);
    const newest = versions.reduce((m, v) => (v.n > m.n ? v : m), versions[0]);
    if (!newest) throw new Error("This artifact has no version yet.");
    onOpenSession(await grillApi.start(newest.id));
  }

  const primary = briefWaits ? (
    <Button variant="primary" icon="check" loading={busy === "confirm"} disabled={state !== "brief_ready"}
      onClick={() => void act("confirm", () => grillApi.confirmBrief(sessionId), "Could not confirm the brief")}>
      Confirm brief
    </Button>
  ) : (
    // Generate issues is build step 4 (owner decision): disabled, with the reason in a tooltip.
    <Tooltip content={generateTooltip(state, briefConfirmed)}>
      <span tabIndex={0}>
        <Button variant="primary" icon="generate" disabled>
          Generate issues
        </Button>
      </span>
    </Tooltip>
  );

  return (
    <div className="relative flex min-w-0 flex-1 gap-3">
      <main className="flex min-w-0 flex-1 flex-col overflow-hidden rounded-panel bg-surface-1">
        <div className="px-10 pt-8 pb-4">
          <PageHeader
            title={`Grill: ${artifactName}`}
            meta={
              <>
                <span>Session {d.session.n}</span>
                <span aria-hidden>·</span>
                <span>{title}</span>
                <span aria-hidden>·</span>
                <span className="tabular-nums">{count(d.session.answers, "answer")} so far</span>
                <span aria-hidden>·</span>
                <SessionStatus status={state} />
              </>
            }
            steps={<PipelineSteps current={state === "confirmed" ? "generate" : "grill"} />}
            actions={
              <>
                {state === "running" && (
                  <Button loading={busy === "stop"} onClick={() => void act("stop", () => grillApi.stop(sessionId), "Could not stop Claude")}>
                    Stop
                  </Button>
                )}
                {(state === "stopped" || state === "failed") && (
                  <Button icon="regenerate" loading={busy === "resume"}
                    onClick={() => void act("resume", () => grillApi.resume(sessionId), "Could not resume the session")}>
                    Resume
                  </Button>
                )}
                {d.adrReview && adrHidden && (
                  <Button variant="secondary" onClick={() => setAdrHidden(false)}>
                    Review ADR
                  </Button>
                )}
                {!panel && (
                  <Button variant="ghost" icon="grill" onClick={() => setPanel(true)}>
                    Sessions
                  </Button>
                )}
                {primary}
              </>
            }
          />
        </div>
        <div ref={chat} className="relative min-h-0 flex-1 overflow-y-auto px-10 pb-4">
          <div className="mx-auto flex max-w-190 flex-col gap-4.5">
            {d.messages.map((m) => (
              <Message key={m.id} m={m} d={d} open={open} chosen={chosen} disabled={!waiting || busy !== null}
                active={adrAsk === null && (d.adrReview === null || adrHidden)} onPick={pick} onToggle={toggle} onWriteOwn={focusComposer}
                onCopy={() => {
                  if (!d.brief) return;
                  navigator.clipboard.writeText(d.brief.content).then(
                    () => notify.success("Brief copied", "Markdown is on your clipboard"),
                    (e) => notify.error("Could not copy the brief", String(e)),
                  );
                }}
                onAskChanges={() => composer.current?.focus()}
              />
            ))}
            {state === "confirmed" && (
              <ChatNote tone="success">Brief confirmed. Generate issues comes in build step 4.</ChatNote>
            )}
          </div>
        </div>
        <Composer
          ref={composer}
          value={text}
          onChange={setText}
          onSend={send}
          placeholder={composerPlaceholder(state, open?.question ?? null)}
          disabled={!waiting || busy !== null}
          thinking={state === "running"}
          hint={<>Runs <code>claude -p</code> in {shortPath(d.projectPath, home)}</>}
        />
      </main>
      {panel && (
        <RightPanel title={`Sessions · ${artifactName}`} onClose={() => setPanel(false)}>
          <SessionList
            sessions={sessions.map((s) => ({
              id: String(s.id),
              title: `Session ${s.n} · ${s.featureName ?? "New feature"}`,
              meta: `${formatWhen(s.createdAt)} · ${count(s.messages, "message")}`,
              status: s.state,
            }))}
            selectedId={String(sessionId)}
            onSelect={(id) => onOpenSession(Number(id))}
          />
          {openSessionExists ? (
            <Tooltip content="Finish or confirm the open session first">
              <span tabIndex={0} className="self-start">
                <Button variant="ghost" icon="add" disabled>
                  New grill (next feature)
                </Button>
              </span>
            </Tooltip>
          ) : (
            <Button variant="ghost" icon="add" className="self-start" loading={busy === "new"}
              onClick={() => void act("new", newGrill, "Could not start the grill")}>
              New grill (next feature)
            </Button>
          )}
          <span className="text-xs font-medium text-fg-subtle">{state === "running" ? "Running now" : "Run log"}</span>
          {d.runs.length > 0 && (
            <RunLog>
              {d.runs.map((r) => (
                <RunLogItem key={r.id} state={r.running ? "running" : r.error ? "failed" : "done"} action={r.command}
                  ms={r.ms ?? undefined} error={r.error ?? undefined} />
              ))}
            </RunLog>
          )}
          {state === "confirmed" && d.feature && (
            <ReadyBox label="Ready to generate">
              <li>1 feature: {d.feature.name}</li>
              <li>
                Brief file: <span className="tabular-nums">{d.brief?.path}</span>
              </li>
            </ReadyBox>
          )}
        </RightPanel>
      )}
      <Dialog
        open={d.adrReview !== null && !adrHidden}
        onOpenChange={(o) => !o && setAdrHidden(true)}
        title="Read the ADR before it is pushed"
        size="lg"
        footer={
          <>
            <Button variant="ghost" disabled={busy === "adr"}
              onClick={() => void act("adr", () => grillApi.adrDecide(sessionId, false), "Could not discard the ADR")}>
              Discard
            </Button>
            <Button variant="primary" loading={busy === "adr"}
              onClick={() => void act("adr", () => grillApi.adrDecide(sessionId, true), "Could not publish the ADR")}>
              Push and open pull request
            </Button>
          </>
        }
      >
        {d.adrReview && (
          <div className="flex flex-col gap-4">
            <p className="text-base text-fg-muted">
              Branch {d.adrReview.branch} into {d.adrReview.base} · pull request "{d.adrReview.prTitle}". Nothing is pushed until
              you press the button, and the app never merges.
            </p>
            {d.adrReview.files.map((f) => (
              <BriefCard key={f.path} source={f.text} label={`${f.path} source`} meta={f.path} />
            ))}
          </div>
        )}
      </Dialog>
      <Dialog
        open={adrAsk !== null}
        onOpenChange={(o) => !o && setAdrAsk(null)}
        title="Make an ADR?"
        size="sm"
        footer={
          <>
            <Button variant="ghost" onClick={() => setAdrAsk(null)}>
              Cancel
            </Button>
            <Button variant="primary" loading={busy === "send"}
              onClick={() => {
                const a = adrAsk;
                if (!a) return;
                void act("send", async () => {
                  await grillApi.answer(sessionId, a.question, a.answer, true);
                  setText("");
                  setAdrAsk(null);
                }, "Could not send the answer");
              }}>
              Make the ADR
            </Button>
          </>
        }
      >
        <p className="text-base text-fg-muted">
          Claude writes the decision. The app then makes the ADR file on a new branch adr/… in a temporary copy and shows it to
          you. Only after you read it and say yes does it push that branch to origin and open a pull request into the default
          branch of {project.repo}. It never merges, and your checkout stays as it is.
        </p>
      </Dialog>
    </div>
  );
}

function Message({
  m,
  d,
  open,
  chosen,
  disabled,
  active,
  onPick,
  onToggle,
  onWriteOwn,
  onCopy,
  onAskChanges,
}: {
  m: GrillMessage;
  d: GrillDetail;
  open: ReturnType<typeof nextQuestion>;
  chosen: number[];
  disabled: boolean;
  /** False while a dialog is open, so keys 1–4 do not reach the card. */
  active: boolean;
  onPick: (index: number) => void;
  onToggle: (index: number) => void;
  onWriteOwn: () => void;
  onCopy: () => void;
  onAskChanges: () => void;
}) {
  const isOpenBlock = d.openBlock?.messageId === m.id;
  const brief = d.brief;
  switch (m.role) {
    case "owner":
      return <ChatMessage role="user">{m.text}</ChatMessage>;
    case "error":
      return <ChatNote tone="error">{m.text}</ChatNote>;
    case "note":
      return <ChatNote tone={m.text.startsWith("ADR step done") ? "success" : "info"}>{m.text}</ChatNote>;
    case "tool":
      // Tool calls show in the RunLog's command, not in the chat; a denial is worth a line.
      return m.text.startsWith("Denied: ") ? <ChatNote tone="info">{m.text}</ChatNote> : null;
    case "claude": {
      const qs = m.questions?.questions ?? [];
      // The latest brief goes with the last message that asks about the brief (it stays in the history).
      const asksBrief = (x: GrillMessage) => (x.questions?.questions ?? []).some((q) => isBriefQuestion(q));
      const lastBriefAsk = [...d.messages].reverse().find((x) => x.role === "claude" && asksBrief(x));
      const showBrief = brief !== null && lastBriefAsk?.id === m.id;
      return (
        <ChatMessage role="claude">
          {m.text && <p>{m.text}</p>}
          {showBrief && brief && (
            <BriefCard
              source={brief.content}
              meta={
                <>
                  v{brief.n} · saved to {brief.path} (not committed)
                </>
              }
              actions={
                <>
                  <Button variant="ghost" size="sm" icon="copy" onClick={onCopy}>
                    Copy
                  </Button>
                  {d.session.state === "brief_ready" && (
                    <Button variant="ghost" size="sm" icon="edit" onClick={onAskChanges}>
                      Ask for changes
                    </Button>
                  )}
                </>
              }
            />
          )}
          {qs.map((q, i) => {
            // Questions of an answered block, and the ones of the open block that have a draft, show as answered.
            const current = isOpenBlock && open?.index === i;
            const pending = isOpenBlock && !(q.question in (d.openBlock?.drafts ?? {}));
            // One question at a time, as the prototype: later questions of the open block wait.
            if (pending && !current) return null;
            return (
              <SuggestedAnswers
                key={q.question}
                index={i + 1}
                total={qs.length}
                header={q.header}
                question={q.question}
                options={q.options.map((o) => ({ label: o.label, description: o.description }))}
                multiSelect={q.multiSelect}
                selected={current ? chosen : []}
                answered={!current && !pending}
                disabled={disabled || !current}
                active={current && active}
                onPick={onPick}
                onToggle={onToggle}
                onWriteOwn={onWriteOwn}
              />
            );
          })}
        </ChatMessage>
      );
    }
  }
}
