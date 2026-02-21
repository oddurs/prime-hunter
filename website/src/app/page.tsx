import { Hero } from "@/components/hero";
import { StatsBar } from "@/components/stats-bar";
import { FeatureGrid } from "@/components/feature-grid";
import { Pipeline } from "@/components/pipeline";
import { PrimeForms } from "@/components/prime-forms";
import { LiveFeed } from "@/components/live-feed";
import { Comparison } from "@/components/comparison";
import { CtaSection } from "@/components/cta-section";

export default function Home() {
  return (
    <>
      <Hero />
      <StatsBar />
      <FeatureGrid />
      <Pipeline />
      <PrimeForms />
      <LiveFeed />
      <Comparison />
      <CtaSection />
    </>
  );
}
