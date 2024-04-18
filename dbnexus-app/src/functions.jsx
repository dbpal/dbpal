import { message } from "@tauri-apps/api/dialog";
function randomString(e) {
  e = e || 32;
  var t = "ABCDEFGHJKMNPQRSTWXYZabcdefhijkmnprstwxyz2345678",
    a = t.length,
    n = "";
  for (i = 0; i < e; i++) n += t.charAt(Math.floor(Math.random() * a));
  return n;
}

function showMsg(msg, options) {
  message(msg, options);
}
function showSysErrMsg(msg, options) {
  message(msg, options);
}



export { showSysErrMsg, randomString, showMsg };
