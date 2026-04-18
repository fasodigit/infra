<#-- Acte Divers (Actes Administratifs Divers) - Burkina Faso -->
<#-- Template FreeMarker pour generation PDF via Flying Saucer -->
<#-- Couvre : certificat de residence, certificat de vie, certificat de celibat, -->
<#-- certificat de non-divorce, certificat de nationalite, legalisation de signature, -->
<#-- certificat de bonne vie et moeurs, attestation de prise en charge, certificat de coutume -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Strict//EN"
  "http://www.w3.org/TR/xhtml1/DTD/xhtml1-strict.dtd">
<html xmlns="http://www.w3.org/1999/xhtml" lang="fr">
<head>
    <meta charset="UTF-8"/>
    <title>${typeActeDivers!'ACTE DIVERS'}</title>
    <style type="text/css">
        @page {
            size: A4;
            margin: 15mm 15mm 15mm 15mm;
        }

        body {
            font-family: 'Times New Roman', Times, serif;
            margin: 0;
            padding: 0;
            color: #000;
            font-size: 12px;
        }

        .container {
            border: 2px solid #000;
            padding: 15px;
        }

        /* ===== EN-TETE (table layout pour Flying Saucer) ===== */
        .header-table {
            width: 100%;
            border: none;
            border-collapse: collapse;
            margin-bottom: 15px;
        }

        .header-table td {
            border: none;
            vertical-align: top;
            padding: 0;
        }

        .header-left {
            text-align: left;
            font-weight: bold;
            font-size: 12px;
            line-height: 1.6;
        }

        .header-right {
            text-align: center;
            font-size: 13px;
        }

        .header-right strong {
            font-size: 14px;
        }

        .motto {
            font-style: italic;
            font-size: 11px;
        }

        /* ===== TITRE ===== */
        .title-section {
            text-align: center;
            margin-bottom: 15px;
        }

        .title-section h1 {
            margin: 5px 0;
            text-decoration: underline;
            font-size: 20px;
        }

        .title-section h2 {
            margin: 10px 0 5px 0;
            text-decoration: underline;
            font-size: 17px;
        }

        .title-section p {
            font-size: 14px;
            margin-top: 10px;
        }

        /* ===== CHAMPS ===== */
        .content {
            margin: 10px 10px;
        }

        .info-row {
            margin-bottom: 10px;
            font-size: 13px;
            line-height: 1.5;
        }

        .label {
            font-weight: bold;
            text-transform: uppercase;
        }

        .value {
            border-bottom: 1px dotted #000;
            padding-left: 10px;
            padding-bottom: 2px;
        }

        /* ===== CONTENU PRINCIPAL ===== */
        .content-block {
            margin: 25px 10px;
            padding: 15px;
            border: 1px solid #ccc;
        }

        .content-block-label {
            font-weight: bold;
            font-size: 13px;
            text-transform: uppercase;
            margin-bottom: 8px;
        }

        .content-block-text {
            font-size: 14px;
            line-height: 1.8;
            text-align: justify;
        }

        /* ===== MENTIONS COMPLEMENTAIRES ===== */
        .mentions-section {
            margin: 20px 10px 0 10px;
            padding: 8px;
            border-left: 2px solid #000;
        }

        .mentions-title {
            font-size: 10px;
            font-weight: bold;
            text-transform: uppercase;
            margin-bottom: 5px;
        }

        .mention-item {
            font-size: 11px;
            font-style: italic;
            color: #555;
            margin-bottom: 2px;
            padding-left: 8px;
        }

        /* ===== CERTIFICATION ===== */
        .certification {
            margin: 30px 10px 0 10px;
            font-size: 13px;
            font-style: italic;
            text-align: center;
            padding: 10px 0;
            border-top: 1px solid #ccc;
            border-bottom: 1px solid #ccc;
        }

        /* ===== PIED DE PAGE (table layout) ===== */
        .footer-table {
            width: 100%;
            border: none;
            border-collapse: collapse;
            margin-top: 20px;
        }

        .footer-table td {
            border: none;
            vertical-align: top;
            padding: 0;
        }

        .stamp-box {
            border: 1px solid #000;
            width: 150px;
            height: 80px;
            text-align: center;
            font-size: 11px;
            padding-top: 20px;
        }

        .signature-section {
            text-align: center;
        }

        .certify-text {
            font-style: italic;
            font-size: 12px;
            margin-bottom: 15px;
        }

        .signature-name {
            font-weight: bold;
            font-size: 13px;
            margin-top: 30px;
        }

        /* ===== DEMANDEUR ===== */
        .demandeur-section {
            margin: 15px 10px 0 10px;
            padding: 8px;
            border: 1px dashed #999;
        }

        .demandeur-title {
            font-size: 11px;
            font-weight: bold;
            text-transform: uppercase;
            margin-bottom: 4px;
        }

        .demandeur-info {
            font-size: 12px;
            color: #333;
        }

        /* ===== QR CODE VERIFICATION ===== */
        .qr-footer {
            margin-top: 30px;
            border-top: 1px solid #ccc;
            padding-top: 8px;
        }

        .qr-footer-table {
            width: 100%;
            border: none;
            border-collapse: collapse;
        }

        .qr-footer-table td {
            border: none;
            vertical-align: middle;
            padding: 3px;
        }

        .qr-text {
            font-size: 8px;
            color: #666;
        }

        .qr-text-bold {
            font-size: 8px;
            color: #666;
            font-weight: bold;
        }
    </style>
</head>
<body>

<div class="container">

    <!-- ==================== EN-TETE ==================== -->
    <table class="header-table">
        <tr>
            <td class="header-left" style="width: 45%;">
                <div>${region!'REGION DU CENTRE'}</div>
                <div>${province!'PROVINCE DU KADIOGO'}</div>
                <div>COMMUNE DE ${commune!'OUAGADOUGOU'}</div>
            </td>
            <td class="header-right" style="width: 55%;">
                <strong>BURKINA FASO</strong><br/>
                <span class="motto">Unit&#233; &#8212; Progr&#232;s &#8212; Justice</span>
            </td>
        </tr>
    </table>

    <!-- ==================== TITRE ==================== -->
    <div class="title-section">
        <h1>ETAT CIVIL</h1>
        <h2>${typeActeDivers!'ACTE DIVERS'}</h2>
        <p>
            <strong>N&#176; ${numero!'______'}</strong>
            du ${dateActe!'____/____/________'}
        </p>
    </div>

    <!-- ==================== CHAMPS BENEFICIAIRE ==================== -->
    <div class="content">
        <div class="info-row">
            <span class="label">Nom :</span>
            <span class="value">${nom!''}</span>
        </div>

        <div class="info-row">
            <span class="label">Prenom(s) :</span>
            <span class="value">${prenoms!''}</span>
        </div>

        <div class="info-row">
            <span class="label"><#if sexe?? && sexe == 'F'>Nee le<#else>Ne le</#if> :</span>
            <span class="value">${dateNaissanceEnLettres!dateNaissance!''}</span>
        </div>

        <div class="info-row">
            <span class="label">A :</span>
            <span class="value">${lieuNaissance!''}</span>
        </div>

        <div class="info-row">
            <span class="label">Nationalite :</span>
            <span class="value">${nationalite!'Burkinab\u00e8'}</span>
        </div>

        <#if profession?? && profession?has_content>
        <div class="info-row">
            <span class="label">Profession :</span>
            <span class="value">${profession}</span>
        </div>
        </#if>

        <#if domicile?? && domicile?has_content>
        <div class="info-row">
            <span class="label">Domicile :</span>
            <span class="value">${domicile}</span>
        </div>
        </#if>

        <#if numeroCnib?? && numeroCnib?has_content>
        <div class="info-row">
            <span class="label">N&#176; CNIB :</span>
            <span class="value">${numeroCnib}</span>
        </div>
        </#if>
    </div>

    <!-- ==================== CONTENU DE L'ACTE ==================== -->
    <div class="content-block">
        <div class="content-block-label">Objet : ${objetActe!''}</div>
        <div class="content-block-text">
            ${contenuPrincipal!''}
        </div>
    </div>

    <#if motif?? && motif?has_content>
    <div style="margin: 10px 10px 0 10px;">
        <div class="info-row">
            <span class="label">Motif :</span>
            <span class="value">${motif}</span>
        </div>
    </div>
    </#if>

    <#if observations?? && observations?has_content>
    <div style="margin: 10px 10px 0 10px;">
        <div class="info-row">
            <span class="label">Observations :</span>
            <span class="value">${observations}</span>
        </div>
    </div>
    </#if>

    <!-- ==================== MENTIONS COMPLEMENTAIRES ==================== -->
    <#if mentionsComplementaires?? && (mentionsComplementaires?size > 0)>
    <div class="mentions-section">
        <div class="mentions-title">Mentions complementaires :</div>
        <#list mentionsComplementaires as mention>
        <div class="mention-item">- ${mention}</div>
        </#list>
    </div>
    </#if>

    <!-- ==================== DEMANDEUR ==================== -->
    <#if nomDemandeur?? && nomDemandeur?has_content>
    <div class="demandeur-section">
        <div class="demandeur-title">Demandeur (si different du beneficiaire) :</div>
        <div class="demandeur-info">
            ${nomDemandeur}<#if lienDemandeur?? && lienDemandeur?has_content> (${lienDemandeur})</#if>
        </div>
    </div>
    </#if>

    <!-- ==================== CERTIFICATION ==================== -->
    <div class="certification">
        Certifie conforme, delivre pour servir et valoir ce que de droit.
    </div>

    <!-- ==================== PIED DE PAGE ==================== -->
    <table class="footer-table">
        <tr>
            <td style="width: 180px; vertical-align: top;">
                <div class="stamp-box">
                    TIMBRE D'ETAT CIVIL<br/>
                    300 FRANCS
                </div>
            </td>
            <td style="padding-left: 30px;">
                <div class="signature-section">
                    <p>Delivre a ${lieuDelivrance!'Ouagadougou'}, le ${dateDelivrance!'____/____/________'}</p>
                    <p><strong>L'Officier de l'Etat Civil Delegue,</strong></p>
                    <#if signatureUrl?? && signatureUrl?has_content>
                    <img src="${signatureUrl}" style="max-width: 150px; max-height: 60px;" alt="Signature"/>
                    </#if>
                    <div class="signature-name">${nomOfficier!''}</div>
                </div>
            </td>
        </tr>
    </table>

    <!-- ==================== QR CODE VERIFICATION ==================== -->
    <#if referenceUnique?? || codeVerification?? || qrCodeUrl??>
    <div class="qr-footer">
        <table class="qr-footer-table">
            <tr>
                <td style="width: 70%;">
                    <div class="qr-text-bold">DOCUMENT OFFICIEL SECURISE</div>
                    <#if referenceUnique?? && referenceUnique?has_content>
                    <div class="qr-text">Reference: ${referenceUnique}</div>
                    </#if>
                    <#if codeVerification?? && codeVerification?has_content>
                    <div class="qr-text">Code de verification: ${codeVerification}</div>
                    </#if>
                    <#if documentHash?? && documentHash?has_content>
                    <div style="font-size: 6pt; color: #666; margin-top: 2px;">Hash: ${documentHash}</div>
                    </#if>
                    <div class="qr-text">Verification: ${verificationUrl!'https://verify.actes.gov.bf'}</div>
                </td>
                <td style="width: 30%; text-align: right;">
                    <#if qrCodeUrl?? && qrCodeUrl?has_content>
                    <img src="${qrCodeUrl}" style="width: 60px; height: 60px;" alt="QR Code"/>
                    <#else>
                    <div class="qr-text">Scannez le QR code</div>
                    <div class="qr-text">pour verifier l'authenticite</div>
                    </#if>
                </td>
            </tr>
        </table>
    </div>
    </#if>

</div>

</body>
</html>
