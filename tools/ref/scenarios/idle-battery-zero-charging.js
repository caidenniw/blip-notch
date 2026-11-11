__seedStorage({ 'blip.nudgeOn': false });
setTimeout(() => __emit('sys', { battery: 0, plugged: true }), 100);
__shotAt = 1000;
