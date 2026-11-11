__seedStorage({ 'blip.nudgeOn': false });
setTimeout(() => __emit('sys', { battery: 20, plugged: false }), 100);
__shotAt = 1000;
