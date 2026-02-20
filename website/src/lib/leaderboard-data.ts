export interface Contributor {
  rank: number;
  username: string;
  team: string;
  credit: number;
  primesFound: number;
  computeHours: number;
}

export const contributors: Contributor[] = [
  { rank: 1, username: "prime_hunter_42", team: "darkreach-core", credit: 184200, primesFound: 312, computeHours: 8400 },
  { rank: 2, username: "mathforge", team: "darkreach-core", credit: 156800, primesFound: 287, computeHours: 7200 },
  { rank: 3, username: "sieve_master", team: "number-crunchers", credit: 132400, primesFound: 241, computeHours: 6100 },
  { rank: 4, username: "fermat_fan", team: "number-crunchers", credit: 98700, primesFound: 198, computeHours: 4500 },
  { rank: 5, username: "gpucluster", team: "hetzner-fleet", credit: 87300, primesFound: 165, computeHours: 4000 },
  { rank: 6, username: "algosmith", team: "darkreach-core", credit: 76500, primesFound: 142, computeHours: 3500 },
  { rank: 7, username: "prime_oracle", team: "solo", credit: 65200, primesFound: 121, computeHours: 3000 },
  { rank: 8, username: "mersenne_dream", team: "number-crunchers", credit: 54800, primesFound: 98, computeHours: 2500 },
  { rank: 9, username: "factorio_prime", team: "hetzner-fleet", credit: 43100, primesFound: 82, computeHours: 2000 },
  { rank: 10, username: "palindrome_pal", team: "solo", credit: 38900, primesFound: 71, computeHours: 1800 },
  { rank: 11, username: "euler_agent", team: "darkreach-core", credit: 31200, primesFound: 58, computeHours: 1400 },
  { rank: 12, username: "compute_knight", team: "hetzner-fleet", credit: 24600, primesFound: 45, computeHours: 1100 },
  { rank: 13, username: "riemann_z", team: "solo", credit: 19800, primesFound: 37, computeHours: 900 },
  { rank: 14, username: "goldbach_99", team: "number-crunchers", credit: 15400, primesFound: 28, computeHours: 700 },
  { rank: 15, username: "twin_seeker", team: "solo", credit: 11200, primesFound: 21, computeHours: 500 },
];

export interface TeamStanding {
  rank: number;
  name: string;
  members: number;
  totalCredit: number;
  totalPrimes: number;
}

export const teamStandings: TeamStanding[] = [
  { rank: 1, name: "darkreach-core", members: 4, totalCredit: 492700, totalPrimes: 799 },
  { rank: 2, name: "number-crunchers", members: 4, totalCredit: 342300, totalPrimes: 554 },
  { rank: 3, name: "hetzner-fleet", members: 3, totalCredit: 155000, totalPrimes: 292 },
  { rank: 4, name: "solo", members: 4, totalCredit: 135100, totalPrimes: 250 },
];

export const leaderboardStats = {
  totalVolunteers: 38,
  totalPrimes: 2847,
  totalComputeHours: 127000,
};
