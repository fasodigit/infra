import { faker, fakerFR } from '@faker-js/faker';

fakerFR.seed(100);
faker.seed(100);

export function randomSiret(): string {
  return faker.string.numeric(14);
}

export function randomAmm(): string {
  return `AMM-BF-${faker.string.numeric(5)}-${faker.string.alpha({
    length: 2,
    casing: 'upper',
  })}`;
}

export function randomLicence(): string {
  return `LIC-BF-${faker.string.numeric(6)}`;
}

export function randomPhone(): string {
  const prefixes = ['70', '71', '72', '73', '74', '75', '76', '77', '78', '79'];
  const prefix = prefixes[faker.number.int({ min: 0, max: prefixes.length - 1 })] ?? '70';
  return `+226${prefix}${faker.string.numeric(6)}`;
}

export function randomEmail(prefix = 'user'): string {
  // Inclut timestamp + random pour éviter les collisions sur seed déterministe.
  const ts = Date.now().toString(36);
  const rand = Math.random().toString(36).slice(2, 8);
  return `${prefix}.${ts}.${rand}@faso-e2e.test`;
}

export function randomPassword(): string {
  return (
    faker.internet.password({ length: 10, memorable: false, pattern: /[A-Za-z0-9]/ }) +
    '!Aa1'
  );
}

export interface OfferDraft {
  title: string;
  description: string;
  priceXof: number;
  quantity: number;
  unit: 'kg' | 'unite' | 'tonne' | 'litre';
  category: string;
}

export function randomOffer(category = 'Poulets'): OfferDraft {
  return {
    title: `${category} - ${faker.commerce.productAdjective()} ${faker.string.alpha({ length: 4, casing: 'upper' })}`,
    description: fakerFR.lorem.paragraph(),
    priceXof: faker.number.int({ min: 1_000, max: 500_000 }),
    quantity: faker.number.int({ min: 1, max: 1000 }),
    unit: faker.helpers.arrayElement(['kg', 'unite', 'tonne', 'litre']),
    category,
  };
}

export interface DemandDraft {
  title: string;
  description: string;
  maxPriceXof: number;
  quantity: number;
  unit: 'kg' | 'unite' | 'tonne' | 'litre';
  category: string;
  location: string;
}

export function randomDemand(category = 'Poulets'): DemandDraft {
  return {
    title: `Recherche ${category}`,
    description: fakerFR.lorem.sentences(2),
    maxPriceXof: faker.number.int({ min: 500, max: 200_000 }),
    quantity: faker.number.int({ min: 1, max: 100 }),
    unit: faker.helpers.arrayElement(['kg', 'unite']),
    category,
    location: faker.helpers.arrayElement(['Ouagadougou', 'Bobo-Dioulasso', 'Koudougou']),
  };
}
