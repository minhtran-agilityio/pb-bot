import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { GRILL_CHANGED, grillApi, type GrillDetail, type GrillSessionSummary } from "./grill";

/** Runs `load` each time Rust reports a grill change (callers also load once on mount). `match` filters by session id. */
function useGrillEvents(load: () => void, match?: (sessionId: number) => boolean) {
  const ref = useRef({ load, match });
  ref.current = { load, match };
  useEffect(() => {
    let stop: (() => void) | undefined;
    let gone = false;
    listen<{ sessionId: number }>(GRILL_CHANGED, (e) => {
      const m = ref.current.match;
      if (!m || m(e.payload.sessionId)) ref.current.load();
    }).then((un) => (gone ? un() : (stop = un)), () => {});
    return () => {
      gone = true;
      stop?.();
    };
  }, []);
}

/** One session, reloaded on every change Rust reports for it. Bursts are coalesced to one load. */
export function useGrillSession(sessionId: number) {
  const [detail, setDetail] = useState<GrillDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const pending = useRef(false);
  const busy = useRef(false);
  const load = useCallback(() => {
    if (busy.current) {
      pending.current = true;
      return;
    }
    busy.current = true;
    grillApi
      .detail(sessionId)
      .then(
        (d) => {
          setDetail(d);
          setError(null);
        },
        (e) => setError(String(e)),
      )
      .finally(() => {
        busy.current = false;
        if (pending.current) {
          pending.current = false;
          load();
        }
      });
  }, [sessionId]);
  useEffect(() => {
    setDetail(null);
  }, [sessionId]);
  useGrillEvents(load, (id) => id === sessionId);
  useEffect(load, [load]);
  return { detail, error, reload: load };
}

/** The sessions of one artifact (the SessionList), newest first. */
export function useGrillSessions(artifactId: number) {
  const [sessions, setSessions] = useState<GrillSessionSummary[]>([]);
  const load = useCallback(() => {
    grillApi.sessions(artifactId).then(setSessions, () => setSessions([]));
  }, [artifactId]);
  useGrillEvents(load);
  useEffect(load, [load]);
  return sessions;
}

/** The newest session per artifact of a project (the ArtifactCard buttons and chips). */
export function useLatestGrills(projectId: number) {
  const [byArtifact, setByArtifact] = useState<Record<number, GrillSessionSummary>>({});
  const load = useCallback(() => {
    grillApi.latest(projectId).then(
      (all) => setByArtifact(Object.fromEntries(all.map((s) => [s.artifactId, s]))),
      () => setByArtifact({}),
    );
  }, [projectId]);
  useGrillEvents(load);
  useEffect(load, [load]);
  return byArtifact;
}
