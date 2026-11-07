// Headless Edge does not advance compositor animations under --virtual-time-budget, so the
// infinite `breathe` on .ambient would be captured at its first frame there only. Pause it at
// that frame in both engines.
__shotAt = 1500;
setTimeout(() => {
  __emit('dictate', 'start');
  document.querySelector('.ambient').style.animationPlayState = 'paused';
  // A never-ending interval stalls headless Edge's frame production; emit a bounded burst.
  for (let t = 100; t <= 800; t += 100) setTimeout(() => __emit('rec:level', 0.05), t);
}, 100);
