import { Hero } from "@/components/hero";
import { StatsBar } from "@/components/stats-bar";
import { FeatureGrid } from "@/components/feature-grid";
import { Pipeline } from "@/components/pipeline";
import { PrimeForms } from "@/components/prime-forms";
import { Discoveries } from "@/components/discoveries";
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
      <Discoveries />
      <Comparison />
      <CtaSection />
    </>
  );
}
