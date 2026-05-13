export class EmoteLookupPolicy {
  constructor(options) {
    var opts = options || {};
    this.providerCooldownMs = opts.providerCooldownMs || 60_000;
    this.nameCooldownMs = opts.nameCooldownMs || 15_000;
    this.now = opts.now || Date.now;
    this.providerCooldownUntil = Object.create(null);
    this.nameCooldownUntil = Object.create(null);
  }

  canLookupName(name) {
    return this.now() >= (this.nameCooldownUntil[name] || 0);
  }

  availableProviders(providers) {
    var now = this.now();
    return providers.filter(
      function (provider) {
        return now >= (this.providerCooldownUntil[provider] || 0);
      }.bind(this),
    );
  }

  recordFailure(provider, name) {
    var now = this.now();
    this.providerCooldownUntil[provider] = now + this.providerCooldownMs;
    this.nameCooldownUntil[name] = now + this.nameCooldownMs;
  }

  recordNameFailure(name) {
    this.nameCooldownUntil[name] = this.now() + this.nameCooldownMs;
  }

  recordProviderSuccess(provider) {
    delete this.providerCooldownUntil[provider];
  }
}
