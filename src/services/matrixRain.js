// Simple Matrix rain effect on a canvas. Returns a cleanup function.
export function startMatrix(canvas) {
  if (!canvas || typeof window === 'undefined') return () => {}

  const ctx = canvas.getContext('2d')
  let width = (canvas.width = window.innerWidth)
  let height = (canvas.height = window.innerHeight)
  const fontSize = 16
  let columns = Math.max(1, Math.floor(width / fontSize))
  let drops = new Array(columns).fill(1)
  const chars = '01░▒▓█ｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿ'
  let frame

  const draw = () => {
    ctx.fillStyle = 'rgba(5, 4, 11, 0.14)'
    ctx.fillRect(0, 0, width, height)

    ctx.fillStyle = 'rgba(57, 255, 20, 0.8)'
    ctx.shadowColor = 'rgba(57, 255, 20, 0.35)'
    ctx.shadowBlur = 8
    ctx.font = `${fontSize}px "IBM Plex Mono", monospace`

    for (let i = 0; i < drops.length; i++) {
      const text = chars.charAt(Math.floor(Math.random() * chars.length))
      ctx.fillText(text, i * fontSize, drops[i] * fontSize)

      if (drops[i] * fontSize > height && Math.random() > 0.975) {
        drops[i] = 0
      }
      drops[i] += 1
    }

    frame = requestAnimationFrame(draw)
  }

  const handleResize = () => {
    width = canvas.width = window.innerWidth
    height = canvas.height = window.innerHeight
    columns = Math.max(1, Math.floor(width / fontSize))
    drops = new Array(columns).fill(1)
  }

  window.addEventListener('resize', handleResize)
  draw()

  return () => {
    cancelAnimationFrame(frame)
    window.removeEventListener('resize', handleResize)
    ctx.clearRect(0, 0, width, height)
  }
}
