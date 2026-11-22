import { computed, ref, watch, type Ref } from "vue";
import type { remoteBackend, RemoteSkill } from "../state";
import type { RemoteToolId } from "../tool-adapters";

interface RemoteSkillsOptions {
  backend: Pick<typeof remoteBackend, "skills" | "enableSkill" | "disableSkill">;
  busy: Readonly<Ref<boolean>>;
  sshConnected: Readonly<Ref<boolean>>;
  activeTargetId: () => string | undefined;
  reviewPreview: () => boolean | undefined;
  perform: (action: () => Promise<void>) => Promise<void>;
}

export function useRemoteSkills({ backend, busy, sshConnected, activeTargetId, reviewPreview, perform }: RemoteSkillsOptions) {
  const skillPending = ref<string>();
  let skillsPollTimer: ReturnType<typeof setTimeout> | undefined;
  let skillsRevision = 0;
  const skills = ref<RemoteSkill[]>([]);
  type RemoteSkillGroup = {
    name: string;
    agents: Array<{ tool: RemoteToolId; label: string; skill?: RemoteSkill }>;
  };
  const skillGroups = computed<RemoteSkillGroup[]>(() => {
    const grouped = new Map<string, Partial<Record<RemoteToolId, RemoteSkill>>>();
    for (const skill of skills.value) {
      const group = grouped.get(skill.name) ?? {};
      group[skill.tool] = skill;
      grouped.set(skill.name, group);
    }
    return [...grouped.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([name, group]) => ({
        name,
        agents: [
          { tool: "codex" as const, label: "Codex", skill: group.codex },
          { tool: "claude" as const, label: "Claude Code", skill: group.claude },
        ],
      }));
  });

  async function refreshSkills() {
    if (reviewPreview()) {
      skills.value = sshConnected.value ? [
        { id: "codex|release-notes", tool: "codex", name: "release-notes", hash: "review-1", fileCount: 3, totalSize: 4100, state: "synced", enabled: true },
        { id: "claude|release-notes", tool: "claude", name: "release-notes", hash: "review-2", fileCount: 3, totalSize: 4100, state: "synced", enabled: true },
        { id: "codex|rust-review", tool: "codex", name: "rust-review", hash: "review-3", fileCount: 5, totalSize: 9200, state: "synced", enabled: true },
        { id: "claude|frontend-audit", tool: "claude", name: "frontend-audit", hash: "review-4", fileCount: 4, totalSize: 6800, state: "synced", enabled: true },
      ] : [];
      return;
    }
    if (!sshConnected.value) {
      skills.value = [];
      return;
    }
    const revision = skillsRevision;
    const next = await backend.skills();
    if (revision === skillsRevision) skills.value = next;
  }

  function stopSkillsPolling() {
    clearTimeout(skillsPollTimer);
    skillsPollTimer = undefined;
  }

  function scheduleSkillsPolling() {
    stopSkillsPolling();
    if (reviewPreview() || !sshConnected.value) return;
    const revision = skillsRevision;
    skillsPollTimer = setTimeout(async () => {
      if (revision !== skillsRevision) return;
      if (!busy.value) await refreshSkills().catch(() => undefined);
      if (revision === skillsRevision) scheduleSkillsPolling();
    }, 2200);
  }

  function toggleSkill(skill: RemoteSkill) {
    if (busy.value || !sshConnected.value) return;
    const revision = skillsRevision;
    skillPending.value = skill.id;
    void perform(async () => {
      try {
        const updated = skill.enabled ? await backend.disableSkill(skill.id) : await backend.enableSkill(skill.id);
        if (revision !== skillsRevision) return;
        skills.value = skills.value.map((item) => item.id === updated.id ? updated : item);
        if (!reviewPreview()) await refreshSkills();
      } finally { skillPending.value = undefined; }
    });
  }

  // Invalidate old replies before the queued summary watcher reads the new target.
  watch([sshConnected, activeTargetId], () => {
    skillsRevision += 1;
    skills.value = [];
    scheduleSkillsPolling();
  }, { flush: "sync" });

  function disposeSkills() {
    skillsRevision += 1;
    stopSkillsPolling();
  }

  return { skillGroups, skillPending, refreshSkills, scheduleSkillsPolling, disposeSkills, toggleSkill };
}
