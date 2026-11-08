// Sidebar hotkeys (#211). E and C are a matched pair: E toggles the EEG sidebar, C the Camera
// sidebar (the ` key stays as an alias, src/camera/debug.js). EEG head-pose re-centring moved from
// C to Shift+C. Opening one sidebar closes the other (EegPanel.toggle / camera.togglePanel).
// C only opens the sidebar, like the toolbar button: the webcam starts from Start Camera (#183).
// Pure, so Node can test the routing.

export function panelHotkey(e) {
  if (!e || e.repeat || e.ctrlKey || e.metaKey || e.altKey) return null;
  if (e.code === 'KeyE') return 'eeg';
  if (e.code === 'KeyC') return e.shiftKey ? 'eegCenter' : 'camera';
  return null;
}

// Overlays that hold every game hotkey (#171-#173): title, planet setup, fail, end, the controls
// banner and the calibration wizard.
export const HOTKEY_OVERLAYS = ['titleScreen', 'planetSetup', 'failScreen', 'endScreen', 'banner'];

export function overlayBlocksHotkeys(doc = globalThis.document) {
  if (!doc) return false;
  if (HOTKEY_OVERLAYS.some((id) => doc.getElementById(id)?.hidden === false)) return true;
  const wiz = doc.querySelector?.('.calib-wiz');
  return !!wiz && !wiz.hidden;
}
