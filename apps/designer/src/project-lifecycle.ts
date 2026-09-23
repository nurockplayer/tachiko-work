import { createDurabilityState, type DurabilityState } from "./durability-state.ts";
import type { DesignerProjectHost } from "./host/browser-project-host.ts";
import { projectTransferFromFiles } from "@tachiko-work/browser-client/host/project-transfer";
import { readSingleLocalRoDocument, type LocalDocumentHandle } from "./host/local-document-ingress.ts";
import type { DesignerClient } from "@tachiko-work/browser-client/runtime/client";
import type { OpenedProjection } from "@tachiko-work/browser-client/runtime/protocol";

type ActiveProject = {
  name: string;
  bytes: ArrayBuffer;
  presentation?: string | undefined;
};

type LifecycleShell = {
  isBusy(): boolean;
  setBusy(busy: boolean): void;
  render(): void;
  clearNotice(): void;
  showFailure(title: string, error: unknown): void;
  showSuccess(title: string, message: string): void;
  hasPendingScalarDrafts(): boolean;
  hasEditDrafts(): boolean;
  confirmDiscard(action: string): boolean;
  confirmLocalDiscard(action: string): Promise<boolean>;
  isDestroyed(): boolean;
  installOpenedOccurrence(opened: OpenedProjection): void;
  installClosedOccurrence(): void;
  selectedSavedProject(): string;
  readOpenedPresentation(bytes: ArrayBuffer, presentation?: string): Promise<void>;
  installOpenedPresentation(): Promise<void>;
  revision(): string | null;
  presentation(): string;
  suggestedProjectName(): string;
  savedPresentation(presentation: string): void;
  refreshSavedProjects(preferred?: string): Promise<void>;
  describeProject(bytes: ArrayBuffer): string;
  syncBeforeUnloadGuard(): void;
};

export type ProjectLifecycle = {
  readonly durability: DurabilityState;
  replaceOccurrence(opened: OpenedProjection, durable: boolean): void;
  setColdBootstrapOccurrence(): void;
  clearColdBootstrapOccurrence(): void;
  isColdBootstrapOccurrence(): boolean;
  openSavedProject(): Promise<void>;
  openLocalDocument(handles: readonly LocalDocumentHandle[], ready: Promise<void>): Promise<void>;
  importProjectDirectory(input: HTMLInputElement): Promise<void>;
  save(): Promise<void>;
  saveAs(): Promise<void>;
  close(): Promise<void>;
};

/** Coordinates Designer project lifecycle host effects; semantic admission stays in the runtime. */
export function createProjectLifecycle(
  client: DesignerClient,
  host: DesignerProjectHost,
  shell: LifecycleShell,
): ProjectLifecycle {
  const durability = createDurabilityState();
  let activeProject: ActiveProject | null = null;
  let coldBootstrapOccurrence = false;

  const setBusy = (busy: boolean): void => {
    shell.setBusy(busy);
    shell.render();
  };

  const replaceOccurrence = (opened: OpenedProjection, durable: boolean): void => {
    shell.installOpenedOccurrence(opened);
    activeProject = null;
    coldBootstrapOccurrence = false;
    durability.install(opened.bootstrap.revision, durable);
    shell.syncBeforeUnloadGuard();
  };

  const rejectBusyLocalDocument = (): void => {
    shell.showFailure("Local file not opened", new Error(
      "Designer is busy with another operation. Try opening the local file again after it completes.",
    ));
    shell.render();
  };

  const saveAs = async (): Promise<void> => {
    const revision = shell.revision();
    if (revision === null || shell.isBusy()) return;
    if (shell.hasEditDrafts()) {
      shell.showFailure("Project not saved", new Error("Apply or cancel pending cell, formula and chart edits before saving."));
      shell.render();
      return;
    }
    const requestedName = window.prompt(
      "Save As a new browser project (existing destinations are never overwritten):",
      shell.suggestedProjectName(),
    );
    if (requestedName === null) return;
    shell.clearNotice();
    setBusy(true);
    try {
      const project = await client.exportProject(revision);
      const presentation = shell.presentation();
      await host.publish(requestedName, project.bytes, presentation);
      activeProject = { name: requestedName.trim(), bytes: project.bytes.slice(0), presentation };
      shell.savedPresentation(presentation);
      durability.published(project.revision);
      shell.syncBeforeUnloadGuard();
      let refreshWarning = "";
      try {
        await shell.refreshSavedProjects(requestedName.trim());
      } catch (error) {
        refreshWarning = ` The project list could not refresh: ${error instanceof Error ? error.message : String(error)}`;
      }
      shell.showSuccess("Save As complete", `${requestedName.trim()} durably committed revision ${project.revision}. ${shell.describeProject(project.bytes)}${refreshWarning}`);
    } catch (error) {
      shell.showFailure("Project not saved", error);
    } finally {
      shell.setBusy(false);
      shell.render();
    }
  };

  return {
    durability,
    replaceOccurrence,
    setColdBootstrapOccurrence: () => { coldBootstrapOccurrence = true; },
    clearColdBootstrapOccurrence: () => { coldBootstrapOccurrence = false; },
    isColdBootstrapOccurrence: () => coldBootstrapOccurrence,
    openSavedProject: async () => {
      const name = shell.selectedSavedProject();
      if (shell.isBusy() || name === "" || !shell.confirmDiscard("Open")) return;
      shell.clearNotice();
      setBusy(true);
      try {
        const snapshot = host.readSnapshot
          ? await host.readSnapshot(name)
          : { bytes: await host.read(name), presentation: undefined };
        // Keep this immutable host snapshot as the future Save CAS base.
        await shell.readOpenedPresentation(snapshot.bytes.slice(0), snapshot.presentation);
        const durableBytes = snapshot.bytes.slice(0);
        replaceOccurrence(await client.openProject(snapshot.bytes), true);
        await shell.installOpenedPresentation();
        activeProject = { name, bytes: durableBytes, presentation: snapshot.presentation };
        shell.showSuccess("Project opened", `${name} is current in a fresh Rust occurrence. ${shell.describeProject(durableBytes)}`);
      } catch (error) {
        shell.showFailure("Project not opened", error);
      } finally {
        shell.setBusy(false);
        shell.render();
      }
    },
    openLocalDocument: async (handles, ready) => {
      if (shell.isBusy()) {
        rejectBusyLocalDocument();
        return;
      }
      try {
        const document = await readSingleLocalRoDocument(handles);
        await ready;
        if (shell.isDestroyed()) return;
        if (shell.isBusy()) {
          rejectBusyLocalDocument();
          return;
        }
        if (!coldBootstrapOccurrence) {
          const nativeConfirmation = handles.length === 1 && handles[0]?.requiresInAppDirtyConfirmation === true;
          const confirmed = nativeConfirmation
            ? await shell.confirmLocalDiscard(`Open '${document.name}'`)
            : shell.confirmDiscard(`Open '${document.name}'`);
          if (!confirmed) return;
        }
        shell.clearNotice();
        setBusy(true);
        try {
          if (!client.openLocalDocument) throw new Error("Local document admission is unavailable.");
          replaceOccurrence(await client.openLocalDocument(document.bytes), true);
          shell.showSuccess("Local file opened", `${document.name} is current in a fresh Rust occurrence.`);
        } catch (error) {
          shell.showFailure("Local file not opened", error);
        } finally {
          shell.setBusy(false);
          shell.render();
        }
      } catch (error) {
        if (!shell.isDestroyed()) {
          shell.showFailure("Local file not opened", error);
          shell.render();
        }
      }
    },
    importProjectDirectory: async (input) => {
      if (shell.isBusy()) return;
      if (!shell.confirmDiscard("Open")) { input.value = ""; return; }
      if (input.files === null) return;
      shell.clearNotice();
      setBusy(true);
      try {
        replaceOccurrence(await client.openProject(await projectTransferFromFiles(input.files)), true);
        shell.showSuccess("Project opened", "The selected canonical .roproj/v1 is current in a fresh Rust occurrence.");
      } catch (error) {
        shell.showFailure("Project not opened", error);
      } finally {
        input.value = "";
        shell.setBusy(false);
        shell.render();
      }
    },
    save: async () => {
      if (activeProject === null) return saveAs();
      const revision = shell.revision();
      if (revision === null || shell.isBusy()) return;
      if (shell.hasEditDrafts()) {
        shell.showFailure("Project not saved", new Error("Apply or cancel pending cell, formula and chart edits before saving."));
        shell.render();
        return;
      }
      shell.clearNotice();
      setBusy(true);
      try {
        if (!host.update) throw new Error("This browser host does not support Save; use Save As.");
        const project = await client.exportProject(revision);
        const presentation = shell.presentation();
        await host.update(activeProject.name, project.bytes, activeProject.bytes, presentation, activeProject.presentation);
        activeProject = { ...activeProject, bytes: project.bytes.slice(0), presentation };
        shell.savedPresentation(presentation);
        durability.published(project.revision);
        shell.showSuccess("Save complete", `${activeProject.name} saved in this browser. ${shell.describeProject(project.bytes)}`);
      } catch (error) {
        shell.showFailure("Project not saved", error);
      } finally {
        shell.setBusy(false);
        shell.syncBeforeUnloadGuard();
        shell.render();
      }
    },
    saveAs,
    close: async () => {
      if (shell.isBusy() || !shell.confirmDiscard("Close")) return;
      shell.clearNotice();
      setBusy(true);
      try {
        await client.closeProject();
        shell.installClosedOccurrence();
        activeProject = null;
        durability.close();
        shell.syncBeforeUnloadGuard();
        shell.showSuccess("Project closed", "The Rust resident occurrence was destroyed. Durable projects are unchanged.");
      } catch (error) {
        shell.showFailure("Project not closed", error);
      } finally {
        shell.setBusy(false);
        shell.render();
      }
    },
  };
}
