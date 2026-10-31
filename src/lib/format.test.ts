import { describe, expect, it } from 'vitest';
import { addDays, amount, moneyInput, parseMoney, parseQty, qty, setMoneyFormat, money, percent } from './format';

describe('money formatting', () => {
  it('formats paisa with grouping', () => {
    expect(amount(125050)).toBe('1,250.50');
    expect(amount(5)).toBe('0.05');
    expect(amount(-99)).toBe('-0.99');
    expect(amount(125050, 0)).toBe('1,251');
  });

  it('adds the currency symbol', () => {
    setMoneyFormat({ symbol: 'Rs', decimals: 2 });
    expect(money(150000)).toBe('Rs 1,500.00');
    expect(money(-500)).toBe('-Rs 5.00');
    expect(money(null)).toBe('—');
  });

  it('parses typed amounts exactly (no floating point drift)', () => {
    expect(parseMoney('1,250.5')).toBe(125050);
    expect(parseMoney('0.1')).toBe(10);
    expect(parseMoney('19.99')).toBe(1999);
    expect(parseMoney('100')).toBe(10000);
    expect(parseMoney('')).toBeNull();
    expect(parseMoney('12.345')).toBeNull();
    expect(parseMoney('abc')).toBeNull();
    expect(moneyInput(125050)).toBe('1250.50');
    expect(moneyInput(10000)).toBe('100');
  });
});

describe('quantities', () => {
  it('formats thousandths', () => {
    expect(qty(3000)).toBe('3');
    expect(qty(1250)).toBe('1.25');
    expect(qty(500)).toBe('0.5');
    expect(qty(1234000)).toBe('1,234');
  });

  it('parses and respects whole-number units', () => {
    expect(parseQty('2')).toBe(2000);
    expect(parseQty('0.75')).toBe(750);
    expect(parseQty('1.5', false)).toBeNull();
    expect(parseQty('3', false)).toBe(3000);
    expect(parseQty('x')).toBeNull();
  });
});

describe('misc', () => {
  it('formats percent and date arithmetic', () => {
    expect(percent(1250)).toBe('12.5%');
    expect(percent(4000)).toBe('40%');
    expect(addDays('2026-11-01', -1)).toBe('2026-10-31');
    expect(addDays('2026-12-31', 1)).toBe('2027-01-01');
  });
});
