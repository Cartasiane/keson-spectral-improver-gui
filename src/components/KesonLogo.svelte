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
    const w = container?.clientWidth || 200
    const h = container?.clientHeight || 140

    scene = new THREE.Scene()
    scene.background = null

    camera = new THREE.PerspectiveCamera(40, w / h, 0.01, 100)
    camera.position.set(0, 0.4, 3.2)

    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true })
    renderer.setSize(w, h)
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.5))
    renderer.outputColorSpace = THREE.SRGBColorSpace
    container.appendChild(renderer.domElement)

    const ambient = new THREE.AmbientLight(0xffffff, 0.9)
    const dir = new THREE.DirectionalLight(0xffffff, 1.0)
    dir.position.set(3, 4, 9)
    scene.add(ambient, dir)

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
      model.scale.setScalar(0.35)
      fitCameraToSphere(camera, box.getBoundingSphere(new THREE.Sphere()))
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
      model.rotation.x = 0.2
    }
    renderer.render(scene, camera)
  }

  function fitCameraToSphere(cam, sphere) {
    const radius = Math.max(1e-4, sphere.radius)
    const dist = radius / Math.sin((cam.fov * Math.PI) / 360)
    const margin = 2.0
    cam.position.set(0, radius * 0.6, dist * margin)
    cam.near = Math.max(0.01, radius * 0.02)
    cam.far = Math.max(cam.near * 50, dist * 4)
    cam.lookAt(0, 0, 0)
    cam.updateProjectionMatrix()
  }
</script>

<div class="logo-wrapper" bind:this={container} aria-label="Keson logo"></div>
