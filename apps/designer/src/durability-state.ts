export type DurabilitySnapshot = {
  current_revision: string | null;
  durable_revision: string | null;
  dirty: boolean;
};

export type DurabilityState = {
  snapshot(): DurabilitySnapshot;
  install(revision: string, durable: boolean): void;
  observe(revision: string): void;
  published(revision: string): void;
  close(): void;
};

/// Track durability by exact resident revision rather than a mutable boolean.
export function createDurabilityState(): DurabilityState {
  let currentRevision: string | null = null;
  let durableRevision: string | null = null;

  return {
    snapshot: () => ({
      current_revision: currentRevision,
      durable_revision: durableRevision,
      dirty: currentRevision !== null && currentRevision !== durableRevision,
    }),
    install: (revision, durable) => {
      currentRevision = revision;
      durableRevision = durable ? revision : null;
    },
    observe: (revision) => {
      currentRevision = revision;
    },
    published: (revision) => {
      durableRevision = revision;
    },
    close: () => {
      currentRevision = null;
      durableRevision = null;
    },
  };
}
