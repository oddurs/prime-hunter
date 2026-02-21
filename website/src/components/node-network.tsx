"use client";

import { useRef, useMemo, useEffect, useState, useCallback } from "react";
import { Canvas, useFrame, useThree } from "@react-three/fiber";
import { PerspectiveCamera } from "@react-three/drei";
import * as THREE from "three";

// ── Scene constants ──────────────────────────────────────────────────
const NODE_COUNT_DESKTOP = 80;
const NODE_COUNT_MOBILE = 40;
const CONNECTION_DIST_DESKTOP = 4.2;
const CONNECTION_DIST_MOBILE = 3.5;
const MAX_CONNECTIONS = 400;
const ROTATION_SPEED = 0.012;
const PULSE_INTERVAL = 2.5; // seconds between discovery pulses
const PULSE_SPEED = 3.0; // how fast pulses travel along edges

const INDIGO = new THREE.Color("#6366f1");
const VIOLET = new THREE.Color("#8b5cf6");
const EMERALD = new THREE.Color("#34d399");

// Weighted color pick: 60% indigo, 25% violet, 15% emerald
function pickColor(): THREE.Color {
  const r = Math.random();
  if (r < 0.6) return INDIGO.clone();
  if (r < 0.85) return VIOLET.clone();
  return EMERALD.clone();
}

// ── Node data ────────────────────────────────────────────────────────
interface NodeData {
  position: THREE.Vector3;
  velocity: THREE.Vector3;
  color: THREE.Color;
  phase: number;
  baseRadius: number; // size variation
  isHub: boolean; // ~12% are "hub" nodes — larger, brighter
}

/** Distribute nodes in a soft ellipsoid — denser toward center */
function createNodes(count: number): NodeData[] {
  return Array.from({ length: count }, (_, i) => {
    // Spherical distribution with gaussian-ish radius
    const theta = Math.random() * Math.PI * 2;
    const phi = Math.acos(2 * Math.random() - 1);
    // Use a mix of uniform + gaussian for natural clustering
    const r = (Math.random() * 0.6 + Math.pow(Math.random(), 0.7) * 0.4);
    const spread = { x: 14, y: 10, z: 5 };
    const x = Math.sin(phi) * Math.cos(theta) * spread.x * r;
    const y = Math.sin(phi) * Math.sin(theta) * spread.y * r;
    const z = Math.cos(phi) * spread.z * r;

    const isHub = i < Math.ceil(count * 0.12);

    return {
      position: new THREE.Vector3(x, y, z),
      velocity: new THREE.Vector3(
        (Math.random() - 0.5) * 0.18,
        (Math.random() - 0.5) * 0.18,
        (Math.random() - 0.5) * 0.08
      ),
      color: isHub ? INDIGO.clone() : pickColor(),
      phase: Math.random() * Math.PI * 2,
      baseRadius: isHub ? 0.12 : 0.04 + Math.random() * 0.04,
      isHub,
    };
  });
}

// ── Pulse state (discovery ripples traveling along edges) ────────────
interface Pulse {
  fromIdx: number;
  toIdx: number;
  t: number; // 0..1 progress
  color: THREE.Color;
}

// ── Inner R3F scene ──────────────────────────────────────────────────
function NetworkScene({ isMobile }: { isMobile: boolean }) {
  const groupRef = useRef<THREE.Group>(null);
  const meshRef = useRef<THREE.InstancedMesh>(null);
  const lineRef = useRef<THREE.LineSegments>(null);
  const pulseRef = useRef<THREE.InstancedMesh>(null);

  const nodeCount = isMobile ? NODE_COUNT_MOBILE : NODE_COUNT_DESKTOP;
  const connectionDist = isMobile ? CONNECTION_DIST_MOBILE : CONNECTION_DIST_DESKTOP;

  const nodes = useMemo(() => createNodes(nodeCount), [nodeCount]);
  const pulsesRef = useRef<Pulse[]>([]);
  const pulseTimerRef = useRef(0);

  const dummy = useMemo(() => new THREE.Object3D(), []);
  const bounds = useMemo(() => ({ x: 14, y: 10, z: 5 }), []);

  // Pre-allocate line geometry buffers
  const linePositions = useMemo(() => new Float32Array(MAX_CONNECTIONS * 6), []);
  const lineColors = useMemo(() => new Float32Array(MAX_CONNECTIONS * 6), []);

  // Store active connection pairs for pulse spawning
  const activeEdgesRef = useRef<[number, number][]>([]);

  const { size } = useThree();

  // Set initial instance matrices + colors
  useEffect(() => {
    if (!meshRef.current) return;
    for (let i = 0; i < nodeCount; i++) {
      const node = nodes[i];
      dummy.position.copy(node.position);
      dummy.scale.setScalar(node.baseRadius / 0.06); // relative to geometry radius
      dummy.updateMatrix();
      meshRef.current.setMatrixAt(i, dummy.matrix);
      meshRef.current.setColorAt(i, node.color);
    }
    meshRef.current.instanceMatrix.needsUpdate = true;
    if (meshRef.current.instanceColor) {
      meshRef.current.instanceColor.needsUpdate = true;
    }
  }, [nodes, nodeCount, dummy]);

  useFrame((_, delta) => {
    if (!meshRef.current || !lineRef.current || !groupRef.current) return;
    const dt = Math.min(delta, 0.05);
    const time = performance.now() * 0.001;

    // Global slow rotation
    groupRef.current.rotation.y += ROTATION_SPEED * dt;

    // Update node positions + per-node breathing
    for (let i = 0; i < nodeCount; i++) {
      const node = nodes[i];

      // Drift
      node.position.x += node.velocity.x * dt;
      node.position.y += node.velocity.y * dt;
      node.position.z += node.velocity.z * dt;

      // Gentle sine bob
      node.position.y += Math.sin(node.phase + time * 0.4) * 0.001;

      // Soft-bounce at ellipsoid bounds
      if (Math.abs(node.position.x) > bounds.x) {
        node.velocity.x *= -1;
        node.position.x = Math.sign(node.position.x) * bounds.x;
      }
      if (Math.abs(node.position.y) > bounds.y) {
        node.velocity.y *= -1;
        node.position.y = Math.sign(node.position.y) * bounds.y;
      }
      if (Math.abs(node.position.z) > bounds.z) {
        node.velocity.z *= -1;
        node.position.z = Math.sign(node.position.z) * bounds.z;
      }

      // Breathing scale — hubs pulse more visibly
      const breathe = node.isHub
        ? 1 + Math.sin(node.phase + time * 1.2) * 0.15
        : 1 + Math.sin(node.phase + time * 0.8) * 0.06;
      const s = (node.baseRadius / 0.06) * breathe;

      dummy.position.copy(node.position);
      dummy.scale.setScalar(s);
      dummy.updateMatrix();
      meshRef.current.setMatrixAt(i, dummy.matrix);
    }
    meshRef.current.instanceMatrix.needsUpdate = true;

    // Build connection lines
    let lineIdx = 0;
    const distSq = connectionDist * connectionDist;
    const edges: [number, number][] = [];

    for (let i = 0; i < nodeCount && lineIdx < MAX_CONNECTIONS; i++) {
      for (let j = i + 1; j < nodeCount && lineIdx < MAX_CONNECTIONS; j++) {
        const dx = nodes[i].position.x - nodes[j].position.x;
        const dy = nodes[i].position.y - nodes[j].position.y;
        const dz = nodes[i].position.z - nodes[j].position.z;
        const d2 = dx * dx + dy * dy + dz * dz;

        if (d2 < distSq) {
          const dist = Math.sqrt(d2);
          const alpha = 1 - dist / connectionDist;
          // Hub connections are slightly brighter
          const hubBoost =
            nodes[i].isHub || nodes[j].isHub ? 1.4 : 1.0;
          const a = alpha * hubBoost;
          const offset = lineIdx * 6;

          linePositions[offset] = nodes[i].position.x;
          linePositions[offset + 1] = nodes[i].position.y;
          linePositions[offset + 2] = nodes[i].position.z;
          linePositions[offset + 3] = nodes[j].position.x;
          linePositions[offset + 4] = nodes[j].position.y;
          linePositions[offset + 5] = nodes[j].position.z;

          const ci = nodes[i].color;
          const cj = nodes[j].color;
          lineColors[offset] = ci.r * a;
          lineColors[offset + 1] = ci.g * a;
          lineColors[offset + 2] = ci.b * a;
          lineColors[offset + 3] = cj.r * a;
          lineColors[offset + 4] = cj.g * a;
          lineColors[offset + 5] = cj.b * a;

          edges.push([i, j]);
          lineIdx++;
        }
      }
    }

    activeEdgesRef.current = edges;

    // Zero out remaining
    for (let k = lineIdx * 6; k < MAX_CONNECTIONS * 6; k++) {
      linePositions[k] = 0;
      lineColors[k] = 0;
    }

    const geom = lineRef.current.geometry;
    geom.attributes.position.needsUpdate = true;
    geom.attributes.color.needsUpdate = true;
    geom.setDrawRange(0, lineIdx * 2);

    // ── Discovery pulses ───────────────────────────────────────────
    pulseTimerRef.current += dt;
    if (
      pulseTimerRef.current > PULSE_INTERVAL &&
      activeEdgesRef.current.length > 0 &&
      pulsesRef.current.length < 6
    ) {
      pulseTimerRef.current = 0;
      // Pick a random hub node as source, find a connected edge
      const hubEdges = activeEdgesRef.current.filter(
        ([a, b]) => nodes[a].isHub || nodes[b].isHub
      );
      const pool = hubEdges.length > 0 ? hubEdges : activeEdgesRef.current;
      const [fromIdx, toIdx] = pool[Math.floor(Math.random() * pool.length)];
      pulsesRef.current.push({
        fromIdx,
        toIdx,
        t: 0,
        color: EMERALD.clone(),
      });
    }

    // Update pulse positions
    const pulseMesh = pulseRef.current;
    if (pulseMesh) {
      const pulses = pulsesRef.current;
      for (let p = pulses.length - 1; p >= 0; p--) {
        pulses[p].t += dt * PULSE_SPEED;
        if (pulses[p].t > 1) {
          pulses.splice(p, 1);
        }
      }
      for (let p = 0; p < 6; p++) {
        if (p < pulses.length) {
          const pulse = pulses[p];
          const from = nodes[pulse.fromIdx].position;
          const to = nodes[pulse.toIdx].position;
          const t = pulse.t;
          dummy.position.set(
            from.x + (to.x - from.x) * t,
            from.y + (to.y - from.y) * t,
            from.z + (to.z - from.z) * t
          );
          // Pulse size peaks at midpoint
          const size = 0.8 + Math.sin(t * Math.PI) * 1.2;
          dummy.scale.setScalar(size);
          dummy.updateMatrix();
          pulseMesh.setMatrixAt(p, dummy.matrix);
          // Fade in/out
          const fade = Math.sin(t * Math.PI);
          const c = pulse.color;
          pulseMesh.setColorAt(
            p,
            new THREE.Color(c.r * fade, c.g * fade, c.b * fade)
          );
        } else {
          dummy.scale.setScalar(0);
          dummy.updateMatrix();
          pulseMesh.setMatrixAt(p, dummy.matrix);
        }
      }
      pulseMesh.instanceMatrix.needsUpdate = true;
      if (pulseMesh.instanceColor) pulseMesh.instanceColor.needsUpdate = true;
    }
  });

  const cameraZ = size.width < 768 ? 18 : 14;

  return (
    <>
      <PerspectiveCamera
        makeDefault
        position={[0, 0, cameraZ]}
        fov={50}
        near={0.1}
        far={100}
      />
      <group ref={groupRef}>
        {/* Nodes — instanced spheres with size variation */}
        <instancedMesh ref={meshRef} args={[undefined, undefined, nodeCount]}>
          <sphereGeometry args={[0.06, 10, 10]} />
          <meshBasicMaterial transparent opacity={0.35} />
        </instancedMesh>

        {/* Connection lines */}
        <lineSegments ref={lineRef}>
          <bufferGeometry>
            <bufferAttribute
              attach="attributes-position"
              args={[linePositions, 3]}
            />
            <bufferAttribute
              attach="attributes-color"
              args={[lineColors, 3]}
            />
          </bufferGeometry>
          <lineBasicMaterial
            vertexColors
            transparent
            opacity={0.12}
            blending={THREE.AdditiveBlending}
            depthWrite={false}
          />
        </lineSegments>

        {/* Discovery pulses — small glowing dots traveling along edges */}
        <instancedMesh ref={pulseRef} args={[undefined, undefined, 6]}>
          <sphereGeometry args={[0.03, 6, 6]} />
          <meshBasicMaterial
            transparent
            opacity={0.4}
            blending={THREE.AdditiveBlending}
            depthWrite={false}
          />
        </instancedMesh>
      </group>
    </>
  );
}

// ── Exported wrapper ─────────────────────────────────────────────────
export function NodeNetwork() {
  const containerRef = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(true);
  const [isMobile, setIsMobile] = useState(false);
  const [reducedMotion, setReducedMotion] = useState(false);

  useEffect(() => {
    const check = () => setIsMobile(window.innerWidth < 768);
    check();
    window.addEventListener("resize", check);
    return () => window.removeEventListener("resize", check);
  }, []);

  useEffect(() => {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    setReducedMotion(mq.matches);
    const handler = (e: MediaQueryListEvent) => setReducedMotion(e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);

  const handleVisibility = useCallback(
    (entries: IntersectionObserverEntry[]) => {
      setVisible(entries[0]?.isIntersecting ?? true);
    },
    []
  );

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const observer = new IntersectionObserver(handleVisibility, {
      threshold: 0.05,
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [handleVisibility]);

  return (
    <div
      ref={containerRef}
      className="absolute inset-0 z-0"
      aria-hidden="true"
    >
      <Canvas
        gl={{
          alpha: true,
          antialias: false,
          powerPreference: "low-power",
        }}
        dpr={[1, 1.5]}
        frameloop={reducedMotion ? "never" : visible ? "always" : "never"}
        style={{ background: "transparent" }}
      >
        <NetworkScene isMobile={isMobile} />
      </Canvas>
    </div>
  );
}
