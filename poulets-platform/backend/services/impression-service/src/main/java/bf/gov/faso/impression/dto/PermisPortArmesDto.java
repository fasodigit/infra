package bf.gov.faso.impression.dto;

import java.time.LocalDate;

/**
 * DTO representant les donnees d'un Permis de Port d'Armes
 * pour le template FreeMarker (Flying Saucer ITextRenderer).
 *
 * Spec ARMES-07 — Burkina Faso.
 */
public record PermisPortArmesDto(

    // ===== NUMERO ET REFERENCE =====
    String numeroPermis,
    String numeroDemande,

    // ===== LOCALISATION ADMINISTRATIVE =====
    String region,
    String province,
    String commune,

    // ===== IDENTITE DU TITULAIRE =====
    String nom,
    String prenoms,
    LocalDate dateNaissance,
    String dateNaissanceEnLettres,
    String lieuNaissance,
    String nationalite,
    String numeroCnib,
    String profession,
    String domicile,
    String telephone,
    String photoUrl,

    // ===== ARME AUTORISEE =====
    String armeCategorie,
    String armeType,
    String armeMarque,
    String armeModele,
    String armeCalibre,
    String armeNumeroSerie,

    // ===== MOTIF =====
    String motifDemande,

    // ===== DATES ET DELIVRANCE =====
    LocalDate dateDelivrance,
    LocalDate dateExpiration,
    String lieuDelivrance,
    String autoriteDelivrance,
    String nomOfficier,

    // ===== RESSOURCES (chemins images) =====
    String signatureUrl,
    String qrCodeUrl,

    // ===== SECURITE ET VERIFICATION =====
    String codeVerification,
    String referenceUnique,
    String documentHash,
    String numeroSerieDocument
) {

    /**
     * Builder statique pour construction flexible du DTO.
     */
    public static Builder builder() {
        return new Builder();
    }

    public static final class Builder {

        // Numero et reference
        private String numeroPermis;
        private String numeroDemande;

        // Localisation administrative
        private String region = "REGION DU CENTRE";
        private String province = "PROVINCE DU KADIOGO";
        private String commune = "OUAGADOUGOU";

        // Identite du titulaire
        private String nom;
        private String prenoms;
        private LocalDate dateNaissance;
        private String dateNaissanceEnLettres;
        private String lieuNaissance;
        private String nationalite = "Burkinab\u00e8";
        private String numeroCnib;
        private String profession;
        private String domicile;
        private String telephone;
        private String photoUrl;

        // Arme autorisee
        private String armeCategorie;
        private String armeType;
        private String armeMarque;
        private String armeModele;
        private String armeCalibre;
        private String armeNumeroSerie;

        // Motif
        private String motifDemande;

        // Dates et delivrance
        private LocalDate dateDelivrance;
        private LocalDate dateExpiration;
        private String lieuDelivrance = "Ouagadougou";
        private String autoriteDelivrance = "Pr\u00e9fecture de Ouagadougou";
        private String nomOfficier;

        // Ressources
        private String signatureUrl;
        private String qrCodeUrl;

        // Securite et verification
        private String codeVerification;
        private String referenceUnique;
        private String documentHash;
        private String numeroSerieDocument;

        private Builder() {}

        // ===== NUMERO ET REFERENCE =====

        public Builder numeroPermis(String numeroPermis) {
            this.numeroPermis = numeroPermis;
            return this;
        }

        public Builder numeroDemande(String numeroDemande) {
            this.numeroDemande = numeroDemande;
            return this;
        }

        // ===== LOCALISATION ADMINISTRATIVE =====

        public Builder region(String region) {
            this.region = region;
            return this;
        }

        public Builder province(String province) {
            this.province = province;
            return this;
        }

        public Builder commune(String commune) {
            this.commune = commune;
            return this;
        }

        // ===== IDENTITE DU TITULAIRE =====

        public Builder nom(String nom) {
            this.nom = nom;
            return this;
        }

        public Builder prenoms(String prenoms) {
            this.prenoms = prenoms;
            return this;
        }

        public Builder dateNaissance(LocalDate dateNaissance) {
            this.dateNaissance = dateNaissance;
            return this;
        }

        public Builder dateNaissanceEnLettres(String dateNaissanceEnLettres) {
            this.dateNaissanceEnLettres = dateNaissanceEnLettres;
            return this;
        }

        public Builder lieuNaissance(String lieuNaissance) {
            this.lieuNaissance = lieuNaissance;
            return this;
        }

        public Builder nationalite(String nationalite) {
            this.nationalite = nationalite;
            return this;
        }

        public Builder numeroCnib(String numeroCnib) {
            this.numeroCnib = numeroCnib;
            return this;
        }

        public Builder profession(String profession) {
            this.profession = profession;
            return this;
        }

        public Builder domicile(String domicile) {
            this.domicile = domicile;
            return this;
        }

        public Builder telephone(String telephone) {
            this.telephone = telephone;
            return this;
        }

        public Builder photoUrl(String photoUrl) {
            this.photoUrl = photoUrl;
            return this;
        }

        // ===== ARME AUTORISEE =====

        public Builder armeCategorie(String armeCategorie) {
            this.armeCategorie = armeCategorie;
            return this;
        }

        public Builder armeType(String armeType) {
            this.armeType = armeType;
            return this;
        }

        public Builder armeMarque(String armeMarque) {
            this.armeMarque = armeMarque;
            return this;
        }

        public Builder armeModele(String armeModele) {
            this.armeModele = armeModele;
            return this;
        }

        public Builder armeCalibre(String armeCalibre) {
            this.armeCalibre = armeCalibre;
            return this;
        }

        public Builder armeNumeroSerie(String armeNumeroSerie) {
            this.armeNumeroSerie = armeNumeroSerie;
            return this;
        }

        // ===== MOTIF =====

        public Builder motifDemande(String motifDemande) {
            this.motifDemande = motifDemande;
            return this;
        }

        // ===== DATES ET DELIVRANCE =====

        public Builder dateDelivrance(LocalDate dateDelivrance) {
            this.dateDelivrance = dateDelivrance;
            return this;
        }

        public Builder dateExpiration(LocalDate dateExpiration) {
            this.dateExpiration = dateExpiration;
            return this;
        }

        public Builder lieuDelivrance(String lieuDelivrance) {
            this.lieuDelivrance = lieuDelivrance;
            return this;
        }

        public Builder autoriteDelivrance(String autoriteDelivrance) {
            this.autoriteDelivrance = autoriteDelivrance;
            return this;
        }

        public Builder nomOfficier(String nomOfficier) {
            this.nomOfficier = nomOfficier;
            return this;
        }

        // ===== RESSOURCES =====

        public Builder signatureUrl(String signatureUrl) {
            this.signatureUrl = signatureUrl;
            return this;
        }

        public Builder qrCodeUrl(String qrCodeUrl) {
            this.qrCodeUrl = qrCodeUrl;
            return this;
        }

        // ===== SECURITE ET VERIFICATION =====

        public Builder codeVerification(String codeVerification) {
            this.codeVerification = codeVerification;
            return this;
        }

        public Builder referenceUnique(String referenceUnique) {
            this.referenceUnique = referenceUnique;
            return this;
        }

        public Builder documentHash(String documentHash) {
            this.documentHash = documentHash;
            return this;
        }

        public Builder numeroSerieDocument(String numeroSerieDocument) {
            this.numeroSerieDocument = numeroSerieDocument;
            return this;
        }

        /**
         * Construit l'instance immutable du DTO.
         * Calcule automatiquement dateExpiration si non fournie
         * (dateDelivrance + 5 ans).
         */
        public PermisPortArmesDto build() {
            LocalDate expiration = this.dateExpiration;
            if (expiration == null && this.dateDelivrance != null) {
                expiration = this.dateDelivrance.plusYears(5);
            }
            return new PermisPortArmesDto(
                this.numeroPermis,
                this.numeroDemande,
                this.region,
                this.province,
                this.commune,
                this.nom,
                this.prenoms,
                this.dateNaissance,
                this.dateNaissanceEnLettres,
                this.lieuNaissance,
                this.nationalite,
                this.numeroCnib,
                this.profession,
                this.domicile,
                this.telephone,
                this.photoUrl,
                this.armeCategorie,
                this.armeType,
                this.armeMarque,
                this.armeModele,
                this.armeCalibre,
                this.armeNumeroSerie,
                this.motifDemande,
                this.dateDelivrance,
                expiration,
                this.lieuDelivrance,
                this.autoriteDelivrance,
                this.nomOfficier,
                this.signatureUrl,
                this.qrCodeUrl,
                this.codeVerification,
                this.referenceUnique,
                this.documentHash,
                this.numeroSerieDocument
            );
        }
    }
}
