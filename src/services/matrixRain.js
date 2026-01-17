// Matrix rain effect
// Returns { cleanup } for control

export function startMatrix(canvas) {
  if (!canvas || typeof window === 'undefined') return { cleanup: () => { } }

  const ctx = canvas.getContext('2d')
  let width = (canvas.width = window.innerWidth)
  let height = (canvas.height = window.innerHeight)

  const baseFontSize = 16
  const spacing = baseFontSize * 2.5
  let columns = Math.max(1, Math.floor(width / spacing))

  let drops = []
  const chars = '01░▒▓█ｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿ'
  let frame

  function initDrops() {
    drops = []
    for (let i = 0; i < columns; i++) {
      const scale = 0.4 + Math.random() * 0.6
      drops.push({
        x: i * spacing + (Math.random() - 0.5) * spacing * 0.3,
        y: Math.random() * height - height,
        speed: scale * 2.5,
        scale: scale,
        trailLength: Math.floor(5 + Math.random() * 15),
        trail: []
      })
    }
  }
  initDrops()

  const draw = () => {
    // Clear background canvas
    ctx.fillStyle = 'rgb(5, 4, 11)'
    ctx.fillRect(0, 0, width, height)
    ctx.shadowBlur = 0

    // Draw regular rain
    for (let i = 0; i < drops.length; i++) {
      const drop = drops[i]
      const fontSize = Math.floor(baseFontSize * drop.scale)

      ctx.font = `${fontSize}px "VT323", monospace`

      if (drop.y >= 0) {
        const newChar = chars.charAt(Math.floor(Math.random() * chars.length))
        drop.trail.unshift({ char: newChar, y: drop.y })
        if (drop.trail.length > drop.trailLength) {
          drop.trail.pop()
        }
      }

      for (let j = 0; j < drop.trail.length; j++) {
        const t = drop.trail[j]
        const fadeAlpha = (1 - j / drop.trailLength)
        const alpha = fadeAlpha * 0.15 * (0.5 + drop.scale)
        ctx.fillStyle = `rgba(255, 20, 200, ${alpha})`
        ctx.fillText(t.char, drop.x, t.y)
      }

      drop.y += drop.speed

      if (drop.y > height + drop.trailLength * fontSize) {
        const newScale = 0.4 + Math.random() * 0.6
        drop.y = -baseFontSize * (5 + Math.random() * 15)
        drop.speed = newScale * 2.5
        drop.scale = newScale
        drop.trailLength = Math.floor(5 + Math.random() * 15)
        drop.trail = []
      }
    }

    frame = requestAnimationFrame(draw)
  }

  const handleResize = () => {
    width = canvas.width = window.innerWidth
    height = canvas.height = window.innerHeight
    columns = Math.max(1, Math.floor(width / spacing))
    initDrops()
  }

  window.addEventListener('resize', handleResize)
  draw()

  const cleanup = () => {
    cancelAnimationFrame(frame)
    window.removeEventListener('resize', handleResize)
    ctx.clearRect(0, 0, width, height)
  }

  return { cleanup }
}

// Global functions for compatibility (no-op now)
export function setGlobalAddFeaturedTrack(fn) { }
export function triggerFeaturedTrack(name) { }
