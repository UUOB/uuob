<script>
  import qrcode from 'qrcode-generator';
  let { text, size = 200 } = $props();
  const svg = $derived.by(() => {
    const qr = qrcode(0, 'M');
    qr.addData(text);
    qr.make();
    return qr.createSvgTag({ cellSize: 4, margin: 0, scalable: true });
  });
</script>

<div class="qr" style="width:{size}px;height:{size}px">{@html svg}</div>

<style>
  .qr { padding: 14px; background: #fff; border-radius: 18px; box-sizing: content-box; }
  .qr :global(svg) { width: 100%; height: 100%; display: block; }
</style>
