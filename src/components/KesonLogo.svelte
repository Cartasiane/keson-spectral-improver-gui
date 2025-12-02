<script>
  import { onMount, onDestroy } from 'svelte'
  import * as THREE from 'three'
  import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
  import { MeshoptDecoder } from 'three/examples/jsm/libs/meshopt_decoder.module.js'

  let container
  let frame
  let renderer
  let scene
  let camera
  let model
  const colors = [0x60b502, 0xf8e503, 0xe081f5, 0xfd3f4b]
  let colorIndex = 0
  let colorTimer

  onMount(() => {
    const w = container?.clientWidth || 240
    const h = container?.clientHeight || 170

    scene = new THREE.Scene()
    scene.background = null

    camera = new THREE.PerspectiveCamera(40, w / h, 0.01, 100)
    camera.position.set(0, 0.3, 2.3)

    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true })
    renderer.setSize(w, h)
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.5))
    renderer.outputColorSpace = THREE.SRGBColorSpace
    renderer.toneMappingExposure = 1.3
    container.appendChild(renderer.domElement)

    const ambient = new THREE.AmbientLight(0xffffff, 1.5)
    const key = new THREE.DirectionalLight(0xffffff, 1.1)
    key.position.set(2, 3, 3)
    const fill = new THREE.DirectionalLight(0xffffff, 0.8)
    fill.position.set(-2, 1, -2.5)
    scene.add(ambient, key, fill)

    const loader = new GLTFLoader()
    loader.setMeshoptDecoder(MeshoptDecoder)
    loader.load('/keson-model.glb', (gltf) => {
      model = gltf.scene
      model.traverse((child) => {
        if (child.isMesh) {
          child.material = new THREE.MeshStandardMaterial({
            color: colors[colorIndex],
            metalness: 0.2,
            roughness: 0.5
          })
        }
      })
      const box = new THREE.Box3().setFromObject(model)
      const center = box.getCenter(new THREE.Vector3())
      model.position.sub(center)
      model.scale.setScalar(1.0)
      scene.add(model)
    })

    startColorCycle()
    animate()
    window.addEventListener('resize', handleResize)
  })

  onDestroy(() => {
    cancelAnimationFrame(frame)
    clearInterval(colorTimer)
    window.removeEventListener('resize', handleResize)
    if (renderer) {
      renderer.dispose()
      container?.removeChild(renderer.domElement)
    }
  })

  function startColorCycle() {
    colorTimer = setInterval(() => {
      colorIndex = (colorIndex + 1) % colors.length
      if (!model) return
      model.traverse((child) => {
        if (child.isMesh && child.material) {
          child.material.color.setHex(colors[colorIndex])
        }
      })
    }, 1200)
  }

  function handleResize() {
    const w = container?.clientWidth || 200
    const h = container?.clientHeight || 140
    camera.aspect = w / h
    camera.updateProjectionMatrix()
    renderer.setSize(w, h)
  }

  function animate() {
    frame = requestAnimationFrame(animate)
    if (model) {
      model.rotation.y += 0.01
      model.rotation.x = 0.18
    }
    renderer.render(scene, camera)
  }
</script>

<div class="logo-wrapper" bind:this={container} aria-label="Keson logo"></div>
