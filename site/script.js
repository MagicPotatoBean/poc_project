
function doupload() {
    let data = document.getElementById("file").files[0];
    let entry = document.getElementById("file").files[0];
    let response = fetch('/' + encodeURIComponent(entry.name), { method: 'PUT', body: data });
    response.then((response) => {
        return response.text();
    }).then((body) => {
        if (body.startsWith("http://")) {
            document.getElementById("link_out").innerHTML = 'File successfully uploaded! Your file is accessible at <a href="' + body + '" target="_blank">' + body + '</a>';
         } else {
            document.getElementById("link_out").innerHTML = body
        }
    });
};
function replace_urls() {
    document.querySelectorAll(".url").forEach((element, key) => {
        element.innerHTML = url();
    })
    document.getElementById('gui-download-guide').innerHTML = "Enter the file ID and name below (the stuff that came after " + url() + ").";
    document.getElementById('gui-delete-guide').innerHTML = "Enter the file ID and name below (the stuff that came after " + url() + ").";
}
function download_id(id) {
    fetch("/" + id)
        .then(resp => resp.status === 200 ? resp.blob() : Promise.reject("Response was not '200 OK'"))
        .then(blob => {
            const url = window.URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.style.display = 'none';
            a.href = url;
            a.download = id;
            document.body.appendChild(a);
            a.click();
            window.URL.revokeObjectURL(url);
            document.getElementById("download-alert").innerHTML = "Successfully downloaded file '" + id + '"'
        })
        .catch(() => alert("Something went wrong, this is likely because the file doesn't exist"));
}
function delete_id(id) {
    fetch("/" + id, {
        method: 'DELETE'
    }).then((resp) => {
        document.getElementById("delete-alert").innerHTML = "Successfully deleted file '" + id + '"'
    })
}
function url() {
    return location.protocol + '//' + location.host + "/"
}
