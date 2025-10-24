

// here we alloc interval ,you can clearInterval via `setTimeout` too
const int = setInterval(() => { console.log("interval running", new Date().toLocaleDateString()) }, 1000);

// push into this array `cleanFns`
cleanFns.push(() => {
    clearInterval(int)
})


// if u don't clean this interval ,it will run forever