(() => {
  const video = document.querySelector("[data-hls-source]");
  const status = document.getElementById("preview-status");

  if (!video || !status) {
    return;
  }

  const source = video.dataset.hlsSource;
  const retryBaseDelay = 1000;
  const retryMaxDelay = 10000;
  let retryAttempt = 0;
  let retryTimer = null;
  let hls = null;

  const retryDelay = () =>
    Math.min(retryBaseDelay * 2 ** retryAttempt, retryMaxDelay);

  const scheduleRetry = (start) => {
    if (retryTimer !== null) {
      return;
    }
    status.textContent = "Reconnecting to camera…";
    retryTimer = window.setTimeout(() => {
      retryTimer = null;
      retryAttempt += 1;
      start();
    }, retryDelay());
  };

  const nativeHls = video.canPlayType("application/vnd.apple.mpegurl");
  if (nativeHls) {
    const startNative = () => {
      video.src = source;
      video.load();
    };
    video.addEventListener("loadedmetadata", () => {
      retryAttempt = 0;
      status.textContent = "Live preview connected.";
    });
    video.addEventListener("error", () => scheduleRetry(startNative));
    startNative();
    return;
  }

  if (typeof Hls === "undefined" || !Hls.isSupported()) {
    status.textContent = "Live preview is unavailable.";
    return;
  }

  const startHls = () => {
    if (hls !== null) {
      hls.destroy();
    }
    hls = new Hls();
    hls.on(Hls.Events.MANIFEST_PARSED, () => {
      retryAttempt = 0;
      status.textContent = "Live preview connected.";
    });
    hls.on(Hls.Events.ERROR, (_event, data) => {
      if (!data.fatal) {
        return;
      }
      if (data.type === Hls.ErrorTypes.MEDIA_ERROR && retryAttempt === 0) {
        retryAttempt = 1;
        hls.recoverMediaError();
        return;
      }
      scheduleRetry(startHls);
    });
    hls.loadSource(source);
    hls.attachMedia(video);
  };

  startHls();
})();
