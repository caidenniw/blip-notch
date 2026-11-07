// Headless Edge does not advance compositor animations under --virtual-time-budget, so the
// infinite `breathe` on .ambient would be captured at its first frame there only. Pause it at
// that frame in both engines.
__shotAt = 1500;
setTimeout(() => {
  __emit('hotkey', 'tap');
  document.querySelector('.ambient').style.animationPlayState = 'paused';
}, 100);
