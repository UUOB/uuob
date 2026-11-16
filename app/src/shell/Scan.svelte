<script>
  // Phones: the camera, full screen, until it sees a QR code. The picture is
  // read here in the page (jsQR), so no camera or scanner software beyond
  // the phone's own web view is needed.
  import { onMount } from 'svelte';

  // done(text) when a code is seen, done(null, error) when it cannot be.
  let { done } = $props();
  let video = $state();

  onMount(() => {
    let stream;
    let timer;
    let closed = false;
    (async () => {
      try {
        const { default: jsQR } = await import('jsqr');
        stream = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: 'environment', width: { ideal: 1280 } },
          audio: false,
        });
        if (closed) return stream.getTracks().forEach((t) => t.stop());
        video.srcObject = stream;
        await video.play();
        const canvas = document.createElement('canvas');
        const ctx = canvas.getContext('2d', { willReadFrequently: true });
        timer = setInterval(() => {
          const { videoWidth: w, videoHeight: h } = video;
          if (!w || !h) return;
          // The whole frame, at a size that is quick to read.
          const scale = Math.min(1, 800 / Math.max(w, h));
          canvas.width = Math.round(w * scale);
          canvas.height = Math.round(h * scale);
          ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
          const pixels = ctx.getImageData(0, 0, canvas.width, canvas.height);
          const found = jsQR(pixels.data, pixels.width, pixels.height, { inversionAttempts: 'dontInvert' });
          if (found?.data) done(found.data);
        }, 150);
      } catch (e) {
        if (closed) return;
        done(
          null,
          new Error(
            e?.name === 'NotAllowedError'
              ? 'UUOB needs the camera to scan. Allow it in the phone’s settings, or type the code.'
              : 'The camera could not be opened. Type the code instead.',
          ),
        );
      }
    })();
    return () => {
      closed = true;
      clearInterval(timer);
      stream?.getTracks().forEach((t) => t.stop());
    };
  });
</script>

<div class="scanner">
  <!-- svelte-ignore a11y_media_has_caption -->
  <video bind:this={video} playsinline muted></video>
  <div class="aim" aria-hidden="true"></div>
  <div class="bar">
    <p>Point the camera at the QR code</p>
    <button type="button" class="btn lg" onclick={() => done(null, new Error(''))}>Cancel</button>
  </div>
</div>

<style>
  .scanner { position: fixed; inset: 0; z-index: 200; background: #000; display: grid; }
  /* Android's web view lets a playing video take touches meant for what is drawn over it. */
  video { grid-area: 1 / 1; width: 100%; height: 100%; object-fit: cover; pointer-events: none; }
  .aim, .bar { position: relative; z-index: 1; }
  .aim {
    grid-area: 1 / 1; place-self: center; width: min(68vw, 300px); aspect-ratio: 1; margin-bottom: 96px;
    border: 3px solid rgba(255, 255, 255, 0.9); border-radius: 28px; box-shadow: 0 0 0 100vmax rgba(0, 0, 0, 0.35);
  }
  .bar {
    grid-area: 1 / 1; align-self: end; display: grid; gap: 14px; justify-items: center;
    padding: 20px 24px calc(28px + env(safe-area-inset-bottom));
  }
  .bar p { margin: 0; color: #fff; font-weight: 590; text-shadow: 0 1px 6px rgba(0, 0, 0, 0.6); }
  .bar .btn { width: min(320px, 100%); background: rgba(255, 255, 255, 0.92); color: #1d1d1f; }
</style>
