function replace_urls() {
  document.querySelectorAll(".url").forEach((element, key) => {
    element.innerHTML = url();
  });
  document.getElementById("gui-download-guide").innerHTML =
    "Enter the file ID and name below (the stuff that came after " +
    url() +
    ").";
  document.getElementById("gui-delete-guide").innerHTML =
    "Enter the file ID and name below (the stuff that came after " +
    url() +
    ").";
}
function url() {
  return location.protocol + "//" + location.host + "/";
}
function on_load() {
  let btn = document.getElementById("bash-close-btn");
  btn.onclick = bash_close_onclick;
  document.getElementById("pathname").innerHTML = document.location.pathname;
  let os = getOS();
  if (os != null) {
    let os_bar_location = document.getElementById("os-opt");
    os_bar_location.outerHTML =
      '<div id="os-display-div"><img src="' +
      os +
      '.png" id="os-display"></div><div class="vertical-bar-div"><p class="vert-bar-display"></p></div>';
  }
  fetch("https://api.ipify.org?format=json")
    .then((response) => response.json())
    .then((data) => {
      document.getElementById("username").innerHTML = data.ip;
    })
    .catch((error) => {
      console.log("Error:", error);
    });
  document.querySelectorAll(".site").forEach((site_url) => {
    site_url.innerHTML = location.host;
  });
}
function getOS() {
  const userAgent = window.navigator.userAgent,
    platform =
      window.navigator?.userAgentData?.platform || window.navigator.platform,
    macosPlatforms = ["macOS", "Macintosh", "MacIntel", "MacPPC", "Mac68K"],
    windowsPlatforms = ["Win32", "Win64", "Windows", "WinCE"],
    iosPlatforms = ["iPhone", "iPad", "iPod"];
  let os = null;

  if (macosPlatforms.indexOf(platform) !== -1) {
    os = "ios";
  } else if (iosPlatforms.indexOf(platform) !== -1) {
    os = "ios";
  } else if (windowsPlatforms.indexOf(platform) !== -1) {
    os = "windows";
  } else if (/Android/.test(userAgent)) {
    os = "android";
  } else if (/Linux/.test(platform)) {
    os = "linux";
  }

  return os;
}
function set_time() {
  document.getElementById("time").innerHTML = formatDate(
    new Date(),
    "d MMM HH:mm",
  );
}
setInterval(set_time, 1000);
function bash_close_onclick() {
  // Todo!
}
// Thanks to gilly3 at https://stackoverflow.com/questions/14638018/current-time-formatting-with-javascript
function formatDate(date, format, utc) {
  var MMMM = [
    "\x00",
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
  ];
  var MMM = [
    "\x01",
    "Jan",
    "Feb",
    "Mar",
    "Apr",
    "May",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Oct",
    "Nov",
    "Dec",
  ];
  var dddd = [
    "\x02",
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
  ];
  var ddd = ["\x03", "Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

  function ii(i, len) {
    var s = i + "";
    len = len || 2;
    while (s.length < len) s = "0" + s;
    return s;
  }

  var y = utc ? date.getUTCFullYear() : date.getFullYear();
  format = format.replace(/(^|[^\\])yyyy+/g, "$1" + y);
  format = format.replace(/(^|[^\\])yy/g, "$1" + y.toString().substr(2, 2));
  format = format.replace(/(^|[^\\])y/g, "$1" + y);

  var M = (utc ? date.getUTCMonth() : date.getMonth()) + 1;
  format = format.replace(/(^|[^\\])MMMM+/g, "$1" + MMMM[0]);
  format = format.replace(/(^|[^\\])MMM/g, "$1" + MMM[0]);
  format = format.replace(/(^|[^\\])MM/g, "$1" + ii(M));
  format = format.replace(/(^|[^\\])M/g, "$1" + M);

  var d = utc ? date.getUTCDate() : date.getDate();
  format = format.replace(/(^|[^\\])dddd+/g, "$1" + dddd[0]);
  format = format.replace(/(^|[^\\])ddd/g, "$1" + ddd[0]);
  format = format.replace(/(^|[^\\])dd/g, "$1" + ii(d));
  format = format.replace(/(^|[^\\])d/g, "$1" + d);

  var H = utc ? date.getUTCHours() : date.getHours();
  format = format.replace(/(^|[^\\])HH+/g, "$1" + ii(H));
  format = format.replace(/(^|[^\\])H/g, "$1" + H);

  var h = H > 12 ? H - 12 : H == 0 ? 12 : H;
  format = format.replace(/(^|[^\\])hh+/g, "$1" + ii(h));
  format = format.replace(/(^|[^\\])h/g, "$1" + h);

  var m = utc ? date.getUTCMinutes() : date.getMinutes();
  format = format.replace(/(^|[^\\])mm+/g, "$1" + ii(m));
  format = format.replace(/(^|[^\\])m/g, "$1" + m);

  var s = utc ? date.getUTCSeconds() : date.getSeconds();
  format = format.replace(/(^|[^\\])ss+/g, "$1" + ii(s));
  format = format.replace(/(^|[^\\])s/g, "$1" + s);

  var f = utc ? date.getUTCMilliseconds() : date.getMilliseconds();
  format = format.replace(/(^|[^\\])fff+/g, "$1" + ii(f, 3));
  f = Math.round(f / 10);
  format = format.replace(/(^|[^\\])ff/g, "$1" + ii(f));
  f = Math.round(f / 10);
  format = format.replace(/(^|[^\\])f/g, "$1" + f);

  var T = H < 12 ? "AM" : "PM";
  format = format.replace(/(^|[^\\])TT+/g, "$1" + T);
  format = format.replace(/(^|[^\\])T/g, "$1" + T.charAt(0));

  var t = T.toLowerCase();
  format = format.replace(/(^|[^\\])tt+/g, "$1" + t);
  format = format.replace(/(^|[^\\])t/g, "$1" + t.charAt(0));

  var tz = -date.getTimezoneOffset();
  var K = utc || !tz ? "Z" : tz > 0 ? "+" : "-";
  if (!utc) {
    tz = Math.abs(tz);
    var tzHrs = Math.floor(tz / 60);
    var tzMin = tz % 60;
    K += ii(tzHrs) + ":" + ii(tzMin);
  }
  format = format.replace(/(^|[^\\])K/g, "$1" + K);

  var day = (utc ? date.getUTCDay() : date.getDay()) + 1;
  format = format.replace(new RegExp(dddd[0], "g"), dddd[day]);
  format = format.replace(new RegExp(ddd[0], "g"), ddd[day]);

  format = format.replace(new RegExp(MMMM[0], "g"), MMMM[M]);
  format = format.replace(new RegExp(MMM[0], "g"), MMM[M]);

  format = format.replace(/\\(.)/g, "$1");

  return format;
}
