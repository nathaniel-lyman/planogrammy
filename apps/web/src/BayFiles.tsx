import { useEffect, useRef, useState, type RefObject } from 'react';
import { Download, FolderOpen } from 'lucide-react';
import type { PlanogramSession } from './session';

const MAX_FILE_BYTES = 8 * 1024 * 1024;

export function BayFiles({ sessionRef, revision, hasProposal, onOpened, onStatus }: {
  sessionRef: RefObject<PlanogramSession | undefined>;
  revision: number | undefined;
  hasProposal: boolean;
  onOpened: () => void;
  onStatus: (status: { message: string; error: string }) => void;
}) {
  const [name, setName] = useState('Untitled bay');
  const [nameInput, setNameInput] = useState(name);
  const [savedRevision, setSavedRevision] = useState(0);
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  const [reading, setReading] = useState(false);
  const [pending, setPending] = useState<{ json?: string; seed?:number; name: string; revision: number }>();
  const scenarioDialog = useRef<HTMLDialogElement>(null);
  const [seed,setSeed]=useState('20260930');
  const saveDialog = useRef<HTMLDialogElement>(null);
  const replaceDialog = useRef<HTMLDialogElement>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const returnFocusAfterRead = useRef(false);
  const openButton = useRef<HTMLButtonElement>(null);
  const saveButton = useRef<HTMLButtonElement>(null);
  const dirty = revision !== undefined && revision !== savedRevision;

  useEffect(() => { onStatus({ message, error }); }, [message, error, onStatus]);

  useEffect(() => {
    const beforeUnload = (event: BeforeUnloadEvent) => {
      if (dirty || hasProposal) { event.preventDefault(); event.returnValue = ''; }
    };
    window.addEventListener('beforeunload', beforeUnload);
    return () => window.removeEventListener('beforeunload', beforeUnload);
  }, [dirty, hasProposal]);

  useEffect(() => {
    if (!reading && returnFocusAfterRead.current) {
      returnFocusAfterRead.current = false;
      if (!replaceDialog.current?.open) openButton.current?.focus();
    }
  }, [reading]);

  const reportError = (cause: unknown) => {
    setError(cause instanceof Error ? cause.message : String(cause));
    setMessage('');
  };

  function download() {
    const session = sessionRef.current;
    if (!session) return;
    try {
      const trimmedName = nameInput.trim();
      const json = session.exportBay(trimmedName);
      const url = URL.createObjectURL(new Blob([json], { type: 'application/json' }));
      const link = document.createElement('a');
      link.href = url;
      link.download = `${trimmedName.replace(/[^a-zA-Z0-9 _-]/g, '_') || 'bay'}.planogrammy.json`;
      document.body.append(link);
      link.click();
      link.remove();
      window.setTimeout(() => URL.revokeObjectURL(url), 30_000);
      setName(trimmedName);
      setSavedRevision(session.documentRevision());
      setError('');
      setMessage(`Download started · Revision ${session.context().revision}${hasProposal ? ' · Pending proposal excluded' : ''}`);
      saveDialog.current?.close();
      saveButton.current?.focus();
    } catch (cause) { reportError(cause); }
  }

  function open(candidate: NonNullable<typeof pending>) {
    try {
      const session = sessionRef.current;
      if (!session) return;
      const openedName = candidate.json !== undefined ? session.restoreBay(candidate.json, candidate.revision) : (session.startCereal(candidate.seed!,candidate.revision), candidate.name);
      setName(openedName);
      if (candidate.json !== undefined) setSavedRevision(session.documentRevision());
      setPending(undefined);
      setError('');
      setMessage(`Opened ${openedName}`);
      replaceDialog.current?.close();
      onOpened();
      openButton.current?.focus();
    } catch (cause) {
      reportError(cause);
      setPending(undefined);
      replaceDialog.current?.close();
      openButton.current?.focus();
    }
  }

  async function read(file: File) {
    returnFocusAfterRead.current = true;
    setReading(true);
    setError('');
    try {
      if (file.size > MAX_FILE_BYTES) throw new Error('Bay files must be 8 MiB or smaller.');
      const json = await file.text();
      const session = sessionRef.current;
      if (!session) return;
      const candidate = { json, name: session.inspectBay(json), revision: session.documentRevision() };
      // Read current session state after asynchronous file I/O, not a stale
      // render: WebMCP or keyboard commands may have run while reading.
      if (candidate.revision !== savedRevision || session.hasPendingProposal()) {
        setPending(candidate);
        replaceDialog.current?.showModal();
      } else open(candidate);
    } catch (cause) { reportError(cause); }
    finally { setReading(false); }
  }

  function cancelOpen() {
    setPending(undefined);
    replaceDialog.current?.close();
    openButton.current?.focus();
  }

  return <div className="bay-files">
    <button disabled={revision === undefined} onClick={()=>scenarioDialog.current?.showModal()}>Cereal challenge</button>
    <div className="bay-document"><strong title={name}>{name}</strong><span aria-live="polite">{dirty ? 'Unsaved changes' : 'No unsaved changes'}</span></div>
    <button ref={saveButton} disabled={revision === undefined || reading} onClick={() => { setNameInput(name); setError(''); saveDialog.current?.showModal(); }}><Download size={16}/>Save bay</button>
    <button ref={openButton} disabled={revision === undefined || reading} onClick={() => fileInput.current?.click()}><FolderOpen size={16}/>{reading ? 'Reading…' : 'Open bay'}</button>
    <input ref={fileInput} type="file" accept=".json,.planogrammy" aria-label="Open bay file" hidden onChange={event => { const file = event.target.files?.[0]; event.target.value = ''; if (file) void read(file); }}/>
    <dialog ref={scenarioDialog} className="bay-dialog" aria-labelledby="cereal-heading"><form onSubmit={event=>{event.preventDefault();const session=sessionRef.current;if(!session)return;const candidate={seed:Number(seed),name:'Cereal 8 to 6',revision:session.documentRevision()};scenarioDialog.current?.close();if(dirty||hasProposal){setPending(candidate);replaceDialog.current?.showModal();}else open(candidate);}}><span className="section-label">Synthetic planning exercise</span><h2 id="cereal-heading">Eight bays into six</h2><p>100 fictional cereal SKUs. Preserve the eight-bay reference, edit six-bay alternatives, and compare space, coverage and replenishment assumptions. No retailer data or sales forecast.</p><label>Scenario seed<input type="number" min="0" max="4294967295" step="1" required value={seed} onChange={e=>setSeed(e.target.value)}/></label><div className="bay-dialog-actions"><button type="button" onClick={()=>scenarioDialog.current?.close()}>Cancel</button><button type="submit" className="file-primary">Start cereal challenge</button></div></form></dialog>
    <dialog ref={saveDialog} className="bay-dialog" aria-labelledby="save-bay-heading">
      <form onSubmit={event => { event.preventDefault(); download(); }}>
        <span className="section-label">Portable bay file</span>
        <h2 id="save-bay-heading">Save this arrangement</h2>
        <p>Download the committed bay, catalog and undo history. Open this file to continue editing. Keep the download before closing the browser.</p>
        <label>Bay name<input autoFocus required maxLength={100} value={nameInput} onChange={event => setNameInput(event.target.value)}/></label>
        {hasProposal && <p className="file-proposal-note">Your pending proposal is not committed and will not be included. Accept it first if you want to save those changes.</p>}
        {error && <p role="alert">{error}</p>}
        <div className="bay-dialog-actions"><button type="button" onClick={() => saveDialog.current?.close()}>Cancel</button><button type="submit" className="file-primary">Download bay</button></div>
      </form>
    </dialog>
    <dialog ref={replaceDialog} className="bay-dialog" aria-labelledby="open-bay-heading" onCancel={cancelOpen}>
      <span className="section-label">Open bay file</span>
      <h2 id="open-bay-heading">Replace the current bay?</h2>
      <p>Open <strong>{pending?.name}</strong> and discard {dirty && hasProposal ? 'unsaved changes and the pending proposal' : hasProposal ? 'the pending proposal' : 'unsaved changes'}? Download your current bay first if you want to keep it.</p>
      <div className="bay-dialog-actions"><button autoFocus type="button" onClick={cancelOpen}>Keep current bay</button><button type="button" className="file-primary" onClick={() => pending && open(pending)}>Replace bay</button></div>
    </dialog>
  </div>;
}
