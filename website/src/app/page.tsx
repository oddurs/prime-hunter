import { Navbar } from "@/components/navbar";
import { Hero } from "@/components/hero";
import { StatsBar } from "@/components/stats-bar";
import { PrimeForms } from "@/components/prime-forms";
import { HowItWorks } from "@/components/how-it-works";
import { Discoveries } from "@/components/discoveries";
import { Comparison } from "@/components/comparison";
import { GetStarted } from "@/components/get-started";
import { Footer } from "@/components/footer";

export default function Home() {
  return (
    <>
      <Navbar />
      <main>
        <Hero />
        <StatsBar />
        <PrimeForms />
        <HowItWorks />
        <Discoveries />
        <Comparison />
        <GetStarted />
      </main>
      <Footer />
    </>
  );
}
