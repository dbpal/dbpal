// resource management notice.
// here we alloc interval ,you can clearInterval via `setTimeout` too
const interval = setInterval(() => {
  console.log("interval running", new Date().toLocaleDateString());
}, 1000);
// can push into this array `cleanFns`
// this will show a button on ui to call those functions

setUpClean("call me", () => {
  console.log("call release resource function");
  interval();
});
// if u don't clean this interval ,it will run forever
